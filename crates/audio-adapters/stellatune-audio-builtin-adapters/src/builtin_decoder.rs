use std::fs::File;
use std::io;
use std::path::Path;

use stellatune_audio_core::{AudioFormat, GaplessTrimSpec};
use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::{
    AudioDecoder as SymphoniaDecoder, AudioDecoderOptions as DecoderOptions,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase, Timestamp};

pub(crate) const SOURCE_PENDING_ERROR: &str = "source temporarily pending";
pub(crate) const SOURCE_IO_ERROR_PREFIX: &str = "source I/O failed: ";

fn gapless_trimmed_duration_ms(
    duration_ms: Option<u64>,
    sample_rate: u32,
    gapless_trim_spec: Option<GaplessTrimSpec>,
) -> Option<u64> {
    let duration_ms = duration_ms?;
    let sample_rate = sample_rate.max(1) as u128;
    let trimmed_frames = gapless_trim_spec.map_or(0_u128, |spec| {
        (spec.head_frames as u128).saturating_add(spec.tail_frames as u128)
    });
    let trimmed_ms = trimmed_frames
        .saturating_mul(1000)
        .saturating_add(sample_rate / 2)
        .saturating_div(sample_rate)
        .min(u64::MAX as u128) as u64;
    Some(duration_ms.saturating_sub(trimmed_ms))
}

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

fn locator_path_for_extension(locator: &str) -> &str {
    let trimmed = locator.trim();
    let without_query_or_fragment = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    let Some((scheme, remainder)) = without_query_or_fragment.split_once("://") else {
        return without_query_or_fragment;
    };

    if !matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "file"
    ) {
        return without_query_or_fragment;
    }

    let slash_index = remainder.find('/').unwrap_or(remainder.len());
    &remainder[slash_index..]
}

pub fn extension_from_path(path: &str) -> String {
    Path::new(locator_path_for_extension(path))
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
    spec: AudioFormat,
    duration_ms_hint: Option<u64>,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
    pending: Vec<f32>,
}

impl BuiltinDecoder {
    pub fn open(path: &str) -> Result<Self, String> {
        let opened = open_media_input(path)?;
        let ext = opened.hint_extension;
        if !ext.is_empty() && builtin_decoder_score_for_ext(ext.as_str()).is_none() {
            return Err(format!(
                "builtin decoder does not support extension `{}`",
                if ext.is_empty() {
                    "<none>"
                } else {
                    ext.as_str()
                }
            ));
        }

        Self::open_source(opened.source, ext.as_str())
    }

    pub(crate) fn open_source(
        source: Box<dyn MediaSource>,
        hint_extension: &str,
    ) -> Result<Self, String> {
        let mut hint = Hint::new();
        if !hint_extension.is_empty() {
            hint.with_extension(hint_extension);
        }

        let mss = MediaSourceStream::new(source, MediaSourceStreamOptions::default());

        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| format!("symphonia probe failed: {e}"))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| "missing default audio track".to_string())?;
        let track_id = track.id;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .cloned()
            .ok_or_else(|| "missing audio codec parameters".to_string())?;
        let time_base = track.time_base;
        let num_frames = track.num_frames;
        let encoder_delay_frames = track.delay.unwrap_or(0);
        let encoder_padding_frames = track.padding.unwrap_or(0);

        let mut sample_rate = params.sample_rate.unwrap_or(0);
        let mut channels = params
            .channels
            .as_ref()
            .map(|v| v.count() as u16)
            .unwrap_or(0);

        let mut decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &DecoderOptions::default())
            .map_err(|e| format!("decoder init failed: {e}"))?;

        let mut duration_ms_hint = duration_ms_from_track_params(time_base, num_frames);
        if duration_ms_hint.is_none() {
            // TODO: Re-evaluate whether this seek-based duration fallback should be removed.
            duration_ms_hint = estimate_duration_ms_by_seek(format.as_mut(), track_id, time_base);
            // Restore start position after duration probing.
            let _ = format.seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::ZERO,
                    track_id: Some(track_id),
                },
            );
            decoder.reset();
        }

        let mut pending = Vec::new();
        if sample_rate == 0 || channels == 0 {
            while sample_rate == 0 || channels == 0 {
                match format.next_packet() {
                    Ok(Some(packet)) => {
                        if packet.track_id != track_id {
                            continue;
                        }
                        match decoder.decode(&packet) {
                            Ok(audio_buf) => {
                                if sample_rate == 0 {
                                    sample_rate = audio_buf.spec().rate();
                                }
                                if channels == 0 {
                                    channels = audio_buf.spec().channels().count() as u16;
                                }
                                append_decoded(&mut pending, audio_buf);
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
                    Ok(None) => break,
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
            spec: AudioFormat {
                sample_rate,
                channels,
                channel_mask: None,
            },
            duration_ms_hint,
            encoder_delay_frames,
            encoder_padding_frames,
            pending,
        })
    }

    pub fn spec(&self) -> AudioFormat {
        self.spec
    }

    pub fn duration_ms_hint(&self) -> Option<u64> {
        self.duration_ms_hint
    }

    pub fn effective_duration_ms_hint(&self) -> Option<u64> {
        gapless_trimmed_duration_ms(
            self.duration_ms_hint,
            self.spec.sample_rate,
            self.gapless_trim_spec(),
        )
    }

    pub fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        let spec = GaplessTrimSpec {
            head_frames: self.encoder_delay_frames,
            tail_frames: self.encoder_padding_frames,
        };
        (spec.head_frames != 0 || spec.tail_frames != 0).then_some(spec)
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::from_millis_u64(position_ms),
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|error| match error {
                SymphoniaError::IoError(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    SOURCE_PENDING_ERROR.to_owned()
                },
                SymphoniaError::IoError(error) => {
                    format!("{SOURCE_IO_ERROR_PREFIX}{error}")
                },
                error => format!("seek failed: {error}"),
            })?;
        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        let channels = self.spec.channels.max(1) as usize;
        let want_samples = frames.saturating_mul(channels).max(channels);

        while self.pending.len() < want_samples {
            match self.format.next_packet() {
                Ok(Some(packet)) => {
                    if packet.track_id != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            append_decoded(&mut self.pending, audio_buf);
                        },
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        },
                        Err(e) => return Err(format!("decode failed: {e}")),
                    }
                },
                Ok(None) => break,
                Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    break;
                },
                Err(SymphoniaError::IoError(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Err(SOURCE_PENDING_ERROR.to_owned());
                },
                Err(SymphoniaError::IoError(e)) => {
                    return Err(format!("{SOURCE_IO_ERROR_PREFIX}{e}"));
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

fn append_decoded(pending: &mut Vec<f32>, audio_buf: GenericAudioBufferRef<'_>) {
    let mut samples = vec![0.0f32; audio_buf.samples_interleaved()];
    audio_buf.copy_to_slice_interleaved(&mut samples);
    pending.extend_from_slice(&samples);
}

fn duration_ms_from_track_params(
    time_base: Option<TimeBase>,
    n_frames: Option<u64>,
) -> Option<u64> {
    let tb = time_base?;
    let frames = n_frames?;
    Some(duration_ms_from_time_base(
        tb,
        Timestamp::new(frames.min(i64::MAX as u64) as i64),
    ))
}

fn duration_ms_from_time_base(tb: TimeBase, ts: Timestamp) -> u64 {
    tb.calc_time(ts)
        .map(|time| (time.as_secs_f64() * 1000.0).round() as u64)
        .unwrap_or(u64::MAX)
}

fn open_media_input(locator: &str) -> Result<OpenedMediaInput, String> {
    let file = File::open(locator).map_err(|e| format!("failed to open `{locator}`: {e}"))?;
    Ok(OpenedMediaInput {
        source: Box::new(file),
        hint_extension: extension_from_path(locator),
    })
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
                time: Time::MAX,
                track_id: Some(track_id),
            },
        )
        .ok()?;
    let end_ts = seeked.actual_ts.max(seeked.required_ts);
    Some(duration_ms_from_time_base(tb, end_ts))
}

struct OpenedMediaInput {
    source: Box<dyn MediaSource>,
    hint_extension: String,
}

#[cfg(test)]
mod tests {
    use super::extension_from_path;

    #[test]
    fn extension_from_path_understands_urls() {
        assert_eq!(
            extension_from_path("https://example.com/music/Track.FLAC?token=abc#frag"),
            "flac"
        );
        assert_eq!(extension_from_path("http://example.com/stream"), "");
        assert_eq!(extension_from_path("C:/music/song.mp3"), "mp3");
    }
}
