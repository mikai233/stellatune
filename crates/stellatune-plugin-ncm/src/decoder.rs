use std::io::{Seek, SeekFrom};

use stellatune_plugin_sdk::instance::{
    DecoderDescriptor, DecoderExtScoreRule, DecoderInstance, DecoderOpenArgsRef,
};
use stellatune_plugin_sdk::update::ConfigUpdatable;
use stellatune_plugin_sdk::{
    Decoder, DecoderDescriptor as LegacyDecoderDescriptor, DecoderInfo, DecoderOpenArgs, HostIo,
    SdkError, SdkResult, StAudioSpec, StDecoderInfo, StLogLevel, host_log,
};
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

use crate::flac_offset::find_flac_streaminfo_start;
use crate::io::NcmMediaSource;
use crate::tags::{Tags, apply_revision, build_metadata};

const NCM_MAGIC: &[u8; 8] = b"CTENFDAM";
const NCM_EXT_RULES: &[DecoderExtScoreRule] = &[DecoderExtScoreRule {
    ext: "ncm",
    score: 100,
}];

pub struct NcmDecoder {
    backend: NcmBackend,
}

enum NcmBackend {
    Symphonia(SymphoniaBackend),
}

struct SymphoniaBackend {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    in_channels: usize,
    spec: StAudioSpec,
    sample_buf: Option<SampleBuffer<f32>>,
    pending: Vec<f32>,
    metadata: Option<serde_json::Value>,
    duration_ms: Option<u64>,
    seekable: bool,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
}

impl NcmDecoder {
    fn open_from_io(io: HostIo) -> SdkResult<Self> {
        let mut io_copy = io;
        let file_size = io_copy.size().map_err(|e| e.to_string())?;

        let mut ncm =
            ncmdump::Ncmdump::from_reader(io).map_err(|e| format!("ncmdump parse failed: {e}"))?;

        let audio_start_abs = io_copy
            .stream_position()
            .map_err(|e| format!("tell failed: {e}"))?;
        let len = file_size.saturating_sub(audio_start_abs);

        let info = ncm
            .get_info()
            .map_err(|e| format!("ncmdump get_info failed: {e}"))?;
        let cover = ncm.get_image().ok();
        host_log!(
            StLogLevel::Debug,
            "ncm: container info title={:?} fmt={:?} duration_ms={} cover={}",
            info.name.trim(),
            info.format.trim(),
            info.duration,
            cover.as_ref().map(|b| !b.is_empty()).unwrap_or(false)
        );

        ncm.seek(SeekFrom::Start(0))
            .map_err(|e| format!("ncmdump seek to audio start failed: {e}"))?;

        let hint_ext = info.format.trim().to_string();
        let duration_ms_container = Some(info.duration).filter(|v| *v > 0);

        let mut tags = Tags {
            title: Some(info.name.trim().to_string()).filter(|s| !s.is_empty()),
            album: Some(info.album.trim().to_string()).filter(|s| !s.is_empty()),
            artist: if info.artist.is_empty() {
                None
            } else {
                Some(
                    info.artist
                        .iter()
                        .map(|(name, _id)| name.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" / "),
                )
                .filter(|s| !s.is_empty())
            },
            cover: cover.filter(|b| !b.is_empty()),
        };

        let mut start_offset: u64 = 0;
        if hint_ext.eq_ignore_ascii_case("flac") {
            start_offset = find_flac_streaminfo_start(&mut ncm)?;
            if start_offset != 0 {
                host_log!(
                    StLogLevel::Debug,
                    "ncm: flac payload has leading junk, skipping {} bytes",
                    start_offset
                );
                ncm.seek(SeekFrom::Start(start_offset))
                    .map_err(|e| format!("ncmdump seek to flac start failed: {e}"))?;
            }
        }

        let mut hint = Hint::new();
        if !hint_ext.is_empty() {
            hint.with_extension(&hint_ext);
        }

        let len = len.saturating_sub(start_offset);
        let src = NcmMediaSource::new(ncm, start_offset, len);
        let mss = MediaSourceStream::new(Box::new(src), MediaSourceStreamOptions::default());

        let meta_opts = MetadataOptions {
            limit_visual_bytes: symphonia::core::meta::Limit::Maximum(12 * 1024 * 1024),
            ..Default::default()
        };

        let mut probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &meta_opts)
            .map_err(|e| format!("symphonia probe failed: {e}"))?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| "missing audio track".to_string())?;
        let track_id = track.id;
        let params = track.codec_params.clone();

        let sample_rate = params
            .sample_rate
            .ok_or_else(|| "missing sample rate".to_string())?;
        let channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(0);
        let in_channels = channels as usize;
        if in_channels == 0 {
            return Err(SdkError::msg("missing channels"));
        }
        let out_channels: u16 = if in_channels == 1 { 1 } else { 2 };

        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| format!("decoder init failed: {e}"))?;

        let duration_ms = duration_ms_container.or_else(|| {
            params.time_base.and_then(|tb| {
                params.n_frames.map(|n| {
                    let t = tb.calc_time(n);
                    let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
                    ms.round() as u64
                })
            })
        });

        if let Some(mut m) = probed.metadata.get()
            && let Some(rev) = m.skip_to_latest()
        {
            apply_revision(rev, &mut tags);
        }
        {
            let mut m = format.metadata();
            if let Some(rev) = m.skip_to_latest() {
                apply_revision(rev, &mut tags);
            }
        }

        let metadata = build_metadata(tags, duration_ms);
        host_log!(
            StLogLevel::Info,
            "ncm: opened sr={} ch={} hint_ext={:?} duration_ms={:?}",
            sample_rate,
            channels,
            hint_ext,
            duration_ms
        );

        Ok(Self {
            backend: NcmBackend::Symphonia(SymphoniaBackend {
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
                metadata,
                duration_ms,
                seekable: true,
                encoder_delay_frames: params.delay.unwrap_or(0),
                encoder_padding_frames: params.padding.unwrap_or(0),
            }),
        })
    }
}

impl SymphoniaBackend {
    fn info(&self) -> DecoderInfo {
        DecoderInfo {
            spec: self.spec,
            duration_ms: self.duration_ms,
            seekable: self.seekable,
            encoder_delay_frames: self.encoder_delay_frames,
            encoder_padding_frames: self.encoder_padding_frames,
        }
    }

    fn metadata(&self) -> Option<serde_json::Value> {
        self.metadata.clone()
    }

    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()> {
        if !self.seekable {
            return Err(SdkError::msg("seek not supported"));
        }
        let secs = position_ms / 1000;
        let frac = (position_ms % 1000) as f64 / 1000.0;
        let time = Time::new(secs, frac);
        let _ = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| e.to_string())?;
        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)> {
        let want = (frames as usize).saturating_mul(self.spec.channels.max(1) as usize);
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

                            let needs_realloc = match self.sample_buf.as_ref() {
                                None => true,
                                Some(buf) => buf.capacity() < audio_buf.capacity(),
                            };
                            if needs_realloc {
                                self.sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                            }

                            let sb = self.sample_buf.as_mut().expect("just initialized");
                            sb.copy_interleaved_ref(audio_buf);

                            let in_ch = self.in_channels.max(1);
                            let out_ch = self.spec.channels.max(1) as usize;
                            let samples = sb.samples();
                            let frames = samples.len() / in_ch;

                            if in_ch == out_ch {
                                self.pending.extend_from_slice(samples);
                            } else if out_ch == 1 {
                                for i in 0..frames {
                                    let l = samples[i * in_ch];
                                    let r = samples[i * in_ch + 1];
                                    self.pending.push((l + r) * 0.5);
                                }
                            } else {
                                let extra = (in_ch.saturating_sub(2)) as f32;
                                let norm = 1.0 / (1.0 + 0.5 * extra).max(1.0);
                                for i in 0..frames {
                                    let base = i * in_ch;
                                    let mut l = samples[base];
                                    let mut r = samples[base + 1];
                                    for ch in 2..in_ch {
                                        let v = samples[base + ch];
                                        l += v * 0.5;
                                        r += v * 0.5;
                                    }
                                    self.pending.push((l * norm).clamp(-1.0, 1.0));
                                    self.pending.push((r * norm).clamp(-1.0, 1.0));
                                }
                            }
                        },
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        },
                        Err(e) => return Err(SdkError::msg(e.to_string())),
                    }
                },
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                },
                Err(e) => return Err(SdkError::msg(e.to_string())),
            }
        }

        if self.pending.is_empty() {
            return Ok((0, true));
        }

        let take = want.min(self.pending.len());
        out_interleaved[..take].copy_from_slice(&self.pending[..take]);
        self.pending.drain(..take);
        Ok(((take / self.spec.channels.max(1) as usize) as u32, false))
    }
}

impl Decoder for NcmDecoder {
    type Metadata = serde_json::Value;

    fn info(&self) -> DecoderInfo {
        match &self.backend {
            NcmBackend::Symphonia(b) => b.info(),
        }
    }

    fn metadata(&self) -> Option<Self::Metadata> {
        match &self.backend {
            NcmBackend::Symphonia(b) => b.metadata(),
        }
    }

    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()> {
        match &mut self.backend {
            NcmBackend::Symphonia(b) => b.seek_ms(position_ms),
        }
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)> {
        match &mut self.backend {
            NcmBackend::Symphonia(b) => b.read_interleaved_f32(frames, out_interleaved),
        }
    }
}

impl LegacyDecoderDescriptor for NcmDecoder {
    const TYPE_ID: &'static str = "ncm";
    const SUPPORTS_SEEK: bool = true;

    fn probe(path_ext: &str, header: &[u8]) -> u8 {
        let ext_ok = path_ext.eq_ignore_ascii_case("ncm");
        let magic_ok = header.len() >= NCM_MAGIC.len() && &header[..NCM_MAGIC.len()] == NCM_MAGIC;
        if ext_ok && magic_ok {
            100
        } else if ext_ok {
            60
        } else if magic_ok {
            80
        } else {
            0
        }
    }

    fn open(args: DecoderOpenArgs<'_>) -> SdkResult<Self> {
        let io = args.io;
        host_log!(
            StLogLevel::Info,
            "ncm: open path={:?} ext={:?} seekable={}",
            args.path,
            args.ext,
            io.is_seekable()
        );
        if !io.is_seekable() {
            return Err(SdkError::msg("ncm: io must be seekable"));
        }
        Self::open_from_io(io)
    }
}

pub struct NcmDecoderInstance {
    inner: Option<NcmDecoder>,
}

impl ConfigUpdatable for NcmDecoderInstance {}

impl DecoderInstance for NcmDecoderInstance {
    fn open(&mut self, args: DecoderOpenArgsRef<'_>) -> SdkResult<()> {
        let host_io = unsafe { HostIo::from_raw(args.io.io_vtable, args.io.io_handle) };
        let decoder = <NcmDecoder as LegacyDecoderDescriptor>::open(DecoderOpenArgs {
            path: args.path_hint,
            ext: args.ext_hint,
            io: host_io,
        })?;
        self.inner = Some(decoder);
        Ok(())
    }

    fn get_info(&self) -> StDecoderInfo {
        self.inner
            .as_ref()
            .map(|d| d.info().to_ffi())
            .unwrap_or(StDecoderInfo {
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
            })
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

impl DecoderDescriptor for NcmDecoderInstance {
    type Config = serde_json::Value;
    type Instance = NcmDecoderInstance;

    const TYPE_ID: &'static str = <NcmDecoder as LegacyDecoderDescriptor>::TYPE_ID;
    const DISPLAY_NAME: &'static str = "NCM Decoder (ncmdump)";
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    const EXT_SCORE_RULES: &'static [DecoderExtScoreRule] = NCM_EXT_RULES;

    fn default_config() -> Self::Config {
        serde_json::Value::Object(serde_json::Map::new())
    }

    fn create(_config: Self::Config) -> SdkResult<Self::Instance> {
        Ok(NcmDecoderInstance { inner: None })
    }
}
