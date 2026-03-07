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
use symphonia::core::meta::{MetadataOptions, StandardVisualKey, Value};
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

#[derive(Debug, Clone, Default)]
pub struct BuiltinDecoderMetadata {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub genres: Vec<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    pub comment: Option<String>,
    pub cover_data: Option<Vec<u8>>,
    pub cover_mime: Option<String>,
}

pub struct BuiltinDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    spec: StreamSpec,
    duration_ms_hint: Option<u64>,
    metadata: BuiltinDecoderMetadata,
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

        let mut probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| format!("symphonia probe failed: {e}"))?;

        let mut metadata = BuiltinDecoderMetadata::default();
        if let Some(mut m) = probed.metadata.get()
            && let Some(rev) = m.skip_to_latest()
        {
            apply_metadata_revision(rev, &mut metadata);
        }

        let mut format = probed.format;
        {
            let mut m = format.metadata();
            if let Some(rev) = m.skip_to_latest() {
                apply_metadata_revision(rev, &mut metadata);
            }
        }
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
            metadata,
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

    pub fn metadata(&self) -> BuiltinDecoderMetadata {
        self.metadata.clone()
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

fn apply_metadata_revision(
    rev: &symphonia::core::meta::MetadataRevision,
    out: &mut BuiltinDecoderMetadata,
) {
    for tag in rev.tags() {
        let key = tag.key.trim().to_ascii_lowercase();
        let value = match value_to_string(&tag.value) {
            Some(value) => value,
            None => continue,
        };

        match key.as_str() {
            "title" | "tracktitle" | "tit2" => {
                if out.title.is_none() {
                    out.title = Some(value);
                }
            },
            "artist" | "tpe1" => {
                if out.artists.is_empty() {
                    out.artists.push(value);
                }
            },
            "album" | "talb" => {
                if out.album.is_none() {
                    out.album = Some(value);
                }
            },
            "albumartist" | "album_artist" | "tpe2" => {
                if out.album_artists.is_empty() {
                    out.album_artists.push(value);
                }
            },
            "genre" | "tcon" => {
                if out.genres.is_empty() {
                    out.genres.push(value);
                }
            },
            "track" | "tracknumber" | "track_num" | "trck" => {
                apply_number_pair(value.as_str(), &mut out.track_number, &mut out.track_total);
            },
            "disc" | "discnumber" | "disc_num" | "tpos" => {
                apply_number_pair(value.as_str(), &mut out.disc_number, &mut out.disc_total);
            },
            "date" | "year" | "tyer" | "tdrc" => {
                if out.year.is_none() {
                    out.year = parse_year(value.as_str());
                }
            },
            "comment" | "description" | "comm" => {
                if out.comment.is_none() {
                    out.comment = Some(value);
                }
            },
            _ => {},
        }
    }

    if out.cover_data.is_none() {
        let front = rev
            .visuals()
            .iter()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover));
        let any = rev.visuals().first();
        if let Some(v) = front.or(any).filter(|v| !v.data.is_empty()) {
            out.cover_data = Some(v.data.as_ref().to_vec());
            let mime = v.media_type.trim();
            if !mime.is_empty() {
                out.cover_mime = Some(mime.to_string());
            }
        }
    }
}

fn value_to_string(v: &Value) -> Option<String> {
    let s = match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    };
    let s = s.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn apply_number_pair(raw: &str, first_out: &mut Option<u32>, second_out: &mut Option<u32>) {
    let (first, second) = parse_number_pair(raw);
    if first_out.is_none() {
        *first_out = first;
    }
    if second_out.is_none() {
        *second_out = second;
    }
}

fn parse_number_pair(raw: &str) -> (Option<u32>, Option<u32>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    if let Some((left, right)) = trimmed.split_once('/') {
        return (parse_u32(left), parse_u32(right));
    }
    (parse_u32(trimmed), None)
}

fn parse_u32(raw: &str) -> Option<u32> {
    raw.trim().parse::<u32>().ok().filter(|v| *v > 0)
}

fn parse_year(raw: &str) -> Option<u32> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let year4 = s.chars().take(4).collect::<String>();
    parse_u32(year4.as_str())
}

#[cfg(test)]
mod tests {
    use super::BuiltinDecoder;

    #[test]
    fn debug_builtin_metadata_from_env() {
        let Some(path) = std::env::var_os("STELLATUNE_DEBUG_METADATA_PATH") else {
            eprintln!("skip: STELLATUNE_DEBUG_METADATA_PATH is not set");
            return;
        };
        let path = path.to_string_lossy().to_string();
        let decoder = BuiltinDecoder::open(path.as_str()).expect("open builtin decoder");
        let metadata = decoder.metadata();
        eprintln!("path={path}");
        eprintln!(
            "title={:?} album={:?} artists={:?} album_artists={:?} genres={:?} track={:?}/{:?} disc={:?}/{:?} year={:?} comment={:?} cover_bytes={}",
            metadata.title,
            metadata.album,
            metadata.artists,
            metadata.album_artists,
            metadata.genres,
            metadata.track_number,
            metadata.track_total,
            metadata.disc_number,
            metadata.disc_total,
            metadata.year,
            metadata.comment,
            metadata.cover_data.as_ref().map(|v| v.len()).unwrap_or(0)
        );
        assert!(
            metadata.title.is_some()
                || metadata.album.is_some()
                || !metadata.artists.is_empty()
                || metadata.track_number.is_some()
                || metadata.disc_number.is_some()
                || metadata.comment.is_some()
                || metadata.cover_data.is_some(),
            "builtin metadata is empty"
        );
    }
}
