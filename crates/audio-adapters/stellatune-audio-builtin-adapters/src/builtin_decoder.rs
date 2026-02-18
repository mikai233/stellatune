use std::fs::File;
use std::io;
use std::path::Path;

use stellatune_audio_core::pipeline::context::{GaplessTrimSpec, StreamSpec};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::audio::{AudioBufferRef, SignalSpec};
use symphonia::core::codecs::{Decoder as SymphoniaDecoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::formats::{SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinDecoderScoreRule {
    pub ext: &'static str,
    pub score: u16,
}

pub const BUILTIN_DECODER_SCORE_RULES: &[BuiltinDecoderScoreRule] = &[
    BuiltinDecoderScoreRule {
        ext: "mp1",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mp2",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mp3",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mpa",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aac",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "alac",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4a",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4b",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4r",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "m4p",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mp4",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "mov",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "3gp",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "3g2",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "caf",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "flac",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "wav",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "wave",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aif",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aiff",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "aifc",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "ogg",
        score: 90,
    },
    BuiltinDecoderScoreRule {
        ext: "oga",
        score: 90,
    },
];

pub fn normalize_extension(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

pub fn extension_from_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(normalize_extension)
        .unwrap_or_default()
}

pub fn builtin_decoder_score_for_ext(ext: &str) -> Option<u16> {
    let ext = normalize_extension(ext);
    if ext.is_empty() {
        return None;
    }
    BUILTIN_DECODER_SCORE_RULES
        .iter()
        .find(|rule| rule.ext == ext)
        .map(|rule| rule.score)
}

pub fn builtin_decoder_supported_extensions() -> Vec<String> {
    let mut out = BUILTIN_DECODER_SCORE_RULES
        .iter()
        .map(|rule| rule.ext.to_string())
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

pub struct BuiltinDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    spec: StreamSpec,
    duration_ms_hint: Option<u64>,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
    sample_buf: Option<SampleBuffer<f32>>,
    pending: Vec<f32>,
}

impl BuiltinDecoder {
    pub fn open(path: &str) -> Result<Self, String> {
        let ext = extension_from_path(path);
        if builtin_decoder_score_for_ext(ext.as_str()).is_none() {
            return Err(format!(
                "builtin decoder does not support extension `{}`",
                if ext.is_empty() {
                    "<none>"
                } else {
                    ext.as_str()
                }
            ));
        }

        let mut hint = Hint::new();
        hint.with_extension(ext.as_str());

        let file = File::open(path).map_err(|e| format!("failed to open `{path}`: {e}"))?;
        let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| format!("symphonia probe failed: {e}"))?;

        let mut format = probed.format;
        let track = format
            .default_track()
            .ok_or_else(|| "missing default audio track".to_string())?;
        let track_id = track.id;
        let params = track.codec_params.clone();

        let mut sample_rate = params.sample_rate.unwrap_or(0);
        let mut channels = params
            .channels
            .as_ref()
            .map(|v| v.count() as u16)
            .unwrap_or(0);

        let mut decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| format!("decoder init failed: {e}"))?;

        let mut duration_ms_hint = duration_ms_from_track_params(params.time_base, params.n_frames);
        if duration_ms_hint.is_none() {
            // TODO: Re-evaluate whether this seek-based duration fallback should be removed.
            duration_ms_hint =
                estimate_duration_ms_by_seek(format.as_mut(), track_id, params.time_base);
            // Restore start position after duration probing.
            let _ = format.seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::new(0, 0.0),
                    track_id: Some(track_id),
                },
            );
            decoder.reset();
        }

        let mut sample_buf: Option<SampleBuffer<f32>> = None;
        let mut pending = Vec::new();
        if sample_rate == 0 || channels == 0 {
            while sample_rate == 0 || channels == 0 {
                match format.next_packet() {
                    Ok(packet) => {
                        if packet.track_id() != track_id {
                            continue;
                        }
                        match decoder.decode(&packet) {
                            Ok(audio_buf) => {
                                if sample_rate == 0 {
                                    sample_rate = audio_buf.spec().rate;
                                }
                                if channels == 0 {
                                    channels = audio_buf.spec().channels.count() as u16;
                                }
                                append_decoded(&mut sample_buf, &mut pending, audio_buf);
                            },
                            Err(SymphoniaError::DecodeError(_)) => continue,
                            Err(SymphoniaError::ResetRequired) => {
                                decoder.reset();
                                continue;
                            },
                            Err(e) => {
                                return Err(format!(
                                    "decode failed while probing stream spec: {e}"
                                ));
                            },
                        }
                    },
                    Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        break;
                    },
                    Err(e) => {
                        return Err(format!("read packet failed while probing stream spec: {e}"));
                    },
                }
            }
        }
        if sample_rate == 0 || channels == 0 {
            return Err(format!(
                "missing stream spec after probe: sample_rate={sample_rate} channels={channels}"
            ));
        }

        Ok(Self {
            format,
            decoder,
            track_id,
            spec: StreamSpec {
                sample_rate,
                channels,
            },
            duration_ms_hint,
            encoder_delay_frames: params.delay.unwrap_or(0),
            encoder_padding_frames: params.padding.unwrap_or(0),
            sample_buf,
            pending,
        })
    }

    pub fn spec(&self) -> StreamSpec {
        self.spec
    }

    pub fn duration_ms_hint(&self) -> Option<u64> {
        self.duration_ms_hint
    }

    pub fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        let spec = GaplessTrimSpec {
            head_frames: self.encoder_delay_frames,
            tail_frames: self.encoder_padding_frames,
        };
        (!spec.is_disabled()).then_some(spec)
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        let secs = position_ms / 1000;
        let frac = (position_ms % 1000) as f64 / 1000.0;
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::new(secs, frac),
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| format!("seek failed: {e}"))?;
        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        let channels = self.spec.channels.max(1) as usize;
        let want_samples = frames.saturating_mul(channels).max(channels);

        while self.pending.len() < want_samples {
            match self.format.next_packet() {
                Ok(packet) => {
                    if packet.track_id() != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            append_decoded(&mut self.sample_buf, &mut self.pending, audio_buf);
                        },
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        },
                        Err(e) => return Err(format!("decode failed: {e}")),
                    }
                },
                Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    break;
                },
                Err(e) => return Err(format!("read packet failed: {e}")),
            }
        }

        if self.pending.is_empty() {
            return Ok(None);
        }
        let take = want_samples.min(self.pending.len());
        let out = self.pending.drain(..take).collect::<Vec<_>>();
        Ok(Some(out))
    }
}

fn append_decoded(
    sample_buf: &mut Option<SampleBuffer<f32>>,
    pending: &mut Vec<f32>,
    audio_buf: AudioBufferRef<'_>,
) {
    let spec = SignalSpec::new(audio_buf.spec().rate, audio_buf.spec().channels);
    let duration = audio_buf.capacity() as u64;
    let needs_realloc = sample_buf
        .as_ref()
        .is_none_or(|buf| buf.capacity() < audio_buf.capacity());
    if needs_realloc {
        *sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
    }

    let Some(sample_buf) = sample_buf.as_mut() else {
        return;
    };
    sample_buf.copy_interleaved_ref(audio_buf);
    pending.extend_from_slice(sample_buf.samples());
}

fn duration_ms_from_track_params(
    time_base: Option<TimeBase>,
    n_frames: Option<u64>,
) -> Option<u64> {
    let tb = time_base?;
    let frames = n_frames?;
    Some(duration_ms_from_time_base(tb, frames))
}

fn duration_ms_from_time_base(tb: TimeBase, ts: u64) -> u64 {
    let t = tb.calc_time(ts);
    let ms = (t.seconds as f64 * 1000.0) + (t.frac * 1000.0);
    ms.round() as u64
}

fn estimate_duration_ms_by_seek(
    format: &mut dyn FormatReader,
    track_id: u32,
    time_base: Option<TimeBase>,
) -> Option<u64> {
    let tb = time_base?;
    let seeked = format
        .seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: Time::new(u64::MAX, 0.0),
                track_id: Some(track_id),
            },
        )
        .ok()?;
    let end_ts = seeked.actual_ts.max(seeked.required_ts);
    Some(duration_ms_from_time_base(tb, end_ts))
}
