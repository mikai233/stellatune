use std::{
    io::{Read, Seek, SeekFrom},
    sync::Mutex,
};

use serde_json::{Map, Value};
use stellatune_plugin_sdk::instance::{DecoderDescriptor, DecoderExtScoreRule, DecoderInstance};
use stellatune_plugin_sdk::update::ConfigUpdatable;
use stellatune_plugin_sdk::{
    Decoder, DecoderDescriptor as LegacyDecoderDescriptor, DecoderInfo, DecoderOpenArgs, HostIo,
    SdkError, SdkResult, StAudioSpec,
};
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::{MetadataOptions, StandardTagKey};
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

const KNOWN_EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "m4a", "aac"];
const STREAM_DECODER_EXT_RULES: &[DecoderExtScoreRule] = &[
    DecoderExtScoreRule {
        ext: "mp3",
        score: 80,
    },
    DecoderExtScoreRule {
        ext: "flac",
        score: 80,
    },
    DecoderExtScoreRule {
        ext: "wav",
        score: 80,
    },
    DecoderExtScoreRule {
        ext: "m4a",
        score: 80,
    },
    DecoderExtScoreRule {
        ext: "aac",
        score: 80,
    },
];

pub struct StreamDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    in_channels: usize,
    spec: StAudioSpec,
    sample_buf: Option<SampleBuffer<f32>>,
    pending: Vec<f32>,
    duration_ms: Option<u64>,
    metadata: Option<Value>,
    seekable: bool,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
}

impl Decoder for StreamDecoder {
    type Metadata = Value;

    fn info(&self) -> DecoderInfo {
        DecoderInfo {
            spec: self.spec,
            duration_ms: self.duration_ms,
            seekable: self.seekable,
            encoder_delay_frames: self.encoder_delay_frames,
            encoder_padding_frames: self.encoder_padding_frames,
        }
    }

    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()> {
        if !self.seekable {
            return Err(SdkError::msg("seek not supported"));
        }
        let secs = position_ms / 1000;
        let frac = (position_ms % 1000) as f64 / 1000.0;
        let time = Time::new(secs, frac);
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| SdkError::msg(format!("seek failed: {e}")))?;
        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    fn metadata(&self) -> Option<Self::Metadata> {
        self.metadata.clone()
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)> {
        let out_channels = self.spec.channels.max(1) as usize;
        let want = (frames as usize).saturating_mul(out_channels);
        if out_interleaved.len() < want {
            return Err(SdkError::msg("output buffer too small"));
        }

        while self.pending.len() < want {
            match self.format.next_packet() {
                Ok(packet) => {
                    if packet.track_id() != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            let spec =
                                SignalSpec::new(audio_buf.spec().rate, audio_buf.spec().channels);
                            let duration = audio_buf.capacity() as u64;
                            let needs_realloc = self
                                .sample_buf
                                .as_ref()
                                .is_none_or(|buf| buf.capacity() < audio_buf.capacity());
                            if needs_realloc {
                                self.sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                            }
                            let Some(sample_buf) = self.sample_buf.as_mut() else {
                                return Err(SdkError::msg("decoder sample buffer unavailable"));
                            };
                            sample_buf.copy_interleaved_ref(audio_buf);
                            push_pending_samples(
                                &mut self.pending,
                                sample_buf.samples(),
                                self.in_channels,
                                out_channels,
                            );
                        },
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        },
                        Err(e) => return Err(SdkError::msg(format!("decode failed: {e}"))),
                    }
                },
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                },
                Err(e) => return Err(SdkError::msg(format!("read packet failed: {e}"))),
            }
        }

        if self.pending.is_empty() {
            return Ok((0, true));
        }

        let take = want.min(self.pending.len());
        out_interleaved[..take].copy_from_slice(&self.pending[..take]);
        self.pending.drain(..take);
        let wrote_frames = (take / out_channels) as u32;
        Ok((wrote_frames, false))
    }
}

impl LegacyDecoderDescriptor for StreamDecoder {
    const TYPE_ID: &'static str = "stream_symphonia";
    const SUPPORTS_SEEK: bool = true;

    fn probe(path_ext: &str, header: &[u8]) -> u8 {
        let ext = path_ext.trim().to_ascii_lowercase();
        if KNOWN_EXTENSIONS.contains(&ext.as_str()) {
            return 80;
        }
        if header.starts_with(b"ID3") || header.starts_with(b"fLaC") || header.starts_with(b"RIFF")
        {
            return 70;
        }
        0
    }

    fn open(args: DecoderOpenArgs<'_>) -> SdkResult<Self> {
        let seekable = args.io.is_seekable();
        let io = args.io;
        let byte_len = io.size().ok();

        let mut hint = Hint::new();
        let mut ext_hint = args.ext.trim().to_ascii_lowercase();
        if ext_hint.is_empty() {
            ext_hint = std::path::Path::new(args.path)
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
        }
        if !ext_hint.is_empty() {
            hint.with_extension(&ext_hint);
        }

        let media_source = IoMediaSource::new(io, byte_len, seekable);
        let mss = MediaSourceStream::new(Box::new(media_source), Default::default());
        let mut probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| SdkError::msg(format!("probe failed: {e}")))?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| SdkError::msg("missing default audio track"))?;
        let track_id = track.id;
        let params = track.codec_params.clone();

        let sample_rate = params
            .sample_rate
            .ok_or_else(|| SdkError::msg("missing sample_rate"))?;
        let in_channels = params.channels.as_ref().map(|v| v.count()).unwrap_or(0);
        if in_channels == 0 {
            return Err(SdkError::msg("missing channels"));
        }
        let out_channels: u16 = if in_channels == 1 { 1 } else { 2 };

        let duration_ms = params.time_base.and_then(|tb| {
            params.n_frames.map(|frames| {
                let t = tb.calc_time(frames);
                let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
                ms.round() as u64
            })
        });

        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| SdkError::msg(format!("decoder init failed: {e}")))?;
        let metadata = {
            let mut title: Option<String> = None;
            let mut artist: Option<String> = None;
            let mut album: Option<String> = None;

            if let Some(mut metadata) = probed.metadata.get()
                && let Some(rev) = metadata.skip_to_latest()
            {
                apply_revision_tags(rev, &mut title, &mut artist, &mut album);
            }
            {
                let mut metadata = format.metadata();
                if let Some(rev) = metadata.skip_to_latest() {
                    apply_revision_tags(rev, &mut title, &mut artist, &mut album);
                }
            }
            build_metadata_json(title, artist, album, duration_ms)
        };

        Ok(Self {
            format,
            decoder,
            track_id,
            in_channels,
            spec: StAudioSpec {
                sample_rate,
                channels: out_channels,
                reserved: 0,
            },
            sample_buf: None,
            pending: Vec::new(),
            duration_ms,
            metadata,
            seekable,
            encoder_delay_frames: params.delay.unwrap_or(0),
            encoder_padding_frames: params.padding.unwrap_or(0),
        })
    }
}

pub struct StreamDecoderInstance {
    inner: Option<StreamDecoder>,
}

impl ConfigUpdatable for StreamDecoderInstance {}

impl DecoderInstance for StreamDecoderInstance {
    fn open(
        &mut self,
        args: stellatune_plugin_sdk::instance::DecoderOpenArgsRef<'_>,
    ) -> SdkResult<()> {
        let host_io = unsafe { HostIo::from_raw(args.io.io_vtable, args.io.io_handle) };
        let decoder = <StreamDecoder as LegacyDecoderDescriptor>::open(DecoderOpenArgs {
            path: args.path_hint,
            ext: args.ext_hint,
            io: host_io,
        })?;
        self.inner = Some(decoder);
        Ok(())
    }

    fn get_info(&self) -> stellatune_plugin_sdk::StDecoderInfo {
        self.inner.as_ref().map(|d| d.info().to_ffi()).unwrap_or(
            stellatune_plugin_sdk::StDecoderInfo {
                spec: StAudioSpec {
                    sample_rate: 0,
                    channels: 0,
                    reserved: 0,
                },
                duration_ms: 0,
                encoder_delay_frames: 0,
                encoder_padding_frames: 0,
                flags: 0,
                reserved: 0,
            },
        )
    }

    fn get_metadata_json(&self) -> SdkResult<Option<String>> {
        let Some(decoder) = self.inner.as_ref() else {
            return Ok(None);
        };
        decoder
            .metadata()
            .map(|v| serde_json::to_string(&v).map_err(SdkError::from))
            .transpose()
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)> {
        let decoder = self
            .inner
            .as_mut()
            .ok_or_else(|| SdkError::msg("decoder is not open"))?;
        decoder.read_interleaved_f32(frames, out_interleaved)
    }

    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()> {
        let decoder = self
            .inner
            .as_mut()
            .ok_or_else(|| SdkError::msg("decoder is not open"))?;
        decoder.seek_ms(position_ms)
    }
}

impl DecoderDescriptor for StreamDecoderInstance {
    type Config = Value;
    type Instance = StreamDecoderInstance;

    const TYPE_ID: &'static str = <StreamDecoder as LegacyDecoderDescriptor>::TYPE_ID;
    const DISPLAY_NAME: &'static str = "Stream Decoder (Symphonia)";
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    const EXT_SCORE_RULES: &'static [DecoderExtScoreRule] = STREAM_DECODER_EXT_RULES;

    fn default_config() -> Self::Config {
        Value::Object(Map::new())
    }

    fn create(_config: Self::Config) -> SdkResult<Self::Instance> {
        Ok(StreamDecoderInstance { inner: None })
    }
}

struct IoMediaSource {
    io: Mutex<HostIo>,
    byte_len: Option<u64>,
    seekable: bool,
}

impl IoMediaSource {
    fn new(io: HostIo, byte_len: Option<u64>, seekable: bool) -> Self {
        Self {
            io: Mutex::new(io),
            byte_len,
            seekable,
        }
    }
}

impl Read for IoMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut io = self
            .io
            .lock()
            .map_err(|_| std::io::Error::other("host io lock poisoned"))?;
        io.read(buf)
    }
}

impl Seek for IoMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let mut io = self
            .io
            .lock()
            .map_err(|_| std::io::Error::other("host io lock poisoned"))?;
        io.seek(pos)
    }
}

impl MediaSource for IoMediaSource {
    fn is_seekable(&self) -> bool {
        self.seekable
    }

    fn byte_len(&self) -> Option<u64> {
        self.byte_len
    }
}

fn push_pending_samples(
    out: &mut Vec<f32>,
    samples: &[f32],
    in_channels: usize,
    out_channels: usize,
) {
    if in_channels == 0 || out_channels == 0 {
        return;
    }
    let frames = samples.len() / in_channels;
    if in_channels == out_channels {
        out.extend_from_slice(samples);
        return;
    }
    if out_channels == 1 {
        for i in 0..frames {
            let l = samples[i * in_channels];
            let r = if in_channels > 1 {
                samples[i * in_channels + 1]
            } else {
                l
            };
            out.push(((l + r) * 0.5).clamp(-1.0, 1.0));
        }
        return;
    }

    let extra = in_channels.saturating_sub(2) as f32;
    let norm = 1.0 / (1.0 + 0.5 * extra).max(1.0);
    for i in 0..frames {
        let base = i * in_channels;
        let mut l = samples[base];
        let mut r = if in_channels > 1 {
            samples[base + 1]
        } else {
            samples[base]
        };
        for ch in 2..in_channels {
            let v = samples[base + ch];
            l += 0.5 * v;
            r += 0.5 * v;
        }
        out.push((l * norm).clamp(-1.0, 1.0));
        out.push((r * norm).clamp(-1.0, 1.0));
    }
}

fn build_metadata_json(
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<u64>,
) -> Option<Value> {
    let mut obj = Map::<String, Value>::new();
    if let Some(v) = title {
        obj.insert("title".to_string(), Value::String(v));
    }
    if let Some(v) = artist {
        obj.insert("artist".to_string(), Value::String(v));
    }
    if let Some(v) = album {
        obj.insert("album".to_string(), Value::String(v));
    }
    if let Some(ms) = duration_ms {
        obj.insert("duration_ms".to_string(), Value::Number(ms.into()));
    }

    if obj.is_empty() {
        None
    } else {
        Some(Value::Object(obj))
    }
}

fn apply_revision_tags(
    rev: &symphonia::core::meta::MetadataRevision,
    title: &mut Option<String>,
    artist: &mut Option<String>,
    album: &mut Option<String>,
) {
    for tag in rev.tags() {
        if title.is_none() && matches!(tag.std_key, Some(StandardTagKey::TrackTitle)) {
            *title = clean_tag_value(&tag.value.to_string());
        } else if artist.is_none() && matches!(tag.std_key, Some(StandardTagKey::Artist)) {
            *artist = clean_tag_value(&tag.value.to_string());
        } else if album.is_none() && matches!(tag.std_key, Some(StandardTagKey::Album)) {
            *album = clean_tag_value(&tag.value.to_string());
        }
    }
}

fn clean_tag_value(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_string())
}
