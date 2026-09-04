use std::fs::File;
use std::io;
use std::path::Path;

use stellatune_audio_core::{
    decoder::GaplessTrimSpec,
    format::{ChannelLayout, PcmFormat, SpeakerPosition},
};
use symphonia::core::audio::{Channels, GenericAudioBufferRef, Position};
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
    spec: PcmFormat,
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
        let mut channel_layout = params
            .channels
            .as_ref()
            .map(channel_layout_from_symphonia)
            .transpose()?;

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
        if sample_rate == 0 || channel_layout.is_none() {
            while sample_rate == 0 || channel_layout.is_none() {
                match format.next_packet() {
                    Ok(Some(packet)) => {
                        if packet.track_id != track_id {
                            continue;
                        }
                        match decoder.decode(&packet) {
                            Ok(audio_buf) => {
                                let decoded_rate = audio_buf.spec().rate();
                                let decoded_layout =
                                    channel_layout_from_symphonia(audio_buf.spec().channels())?;
                                if sample_rate == 0 {
                                    sample_rate = decoded_rate;
                                } else if sample_rate != decoded_rate {
                                    return Err(format!(
                                        "decoded sample rate changed while probing: expected {sample_rate}Hz, got {decoded_rate}Hz"
                                    ));
                                }
                                match channel_layout {
                                    None => channel_layout = Some(decoded_layout),
                                    Some(expected) if expected != decoded_layout => {
                                        return Err(format!(
                                            "decoded channel layout changed while probing: expected {expected:?}, got {decoded_layout:?}",
                                        ));
                                    },
                                    Some(_) => {},
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
        let channel_layout = channel_layout.ok_or_else(|| {
            format!("missing positioned channel layout after probe: sample_rate={sample_rate}")
        })?;
        if sample_rate == 0 {
            return Err(format!(
                "missing stream spec after probe: sample_rate={sample_rate}"
            ));
        }

        Ok(Self {
            format,
            decoder,
            track_id,
            spec: PcmFormat {
                sample_rate,
                channel_layout,
            },
            duration_ms_hint,
            encoder_delay_frames,
            encoder_padding_frames,
            pending,
        })
    }

    pub fn spec(&self) -> PcmFormat {
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
        let channels = usize::from(self.spec.channel_layout.channel_count());
        let want_samples = frames.saturating_mul(channels).max(channels);

        while self.pending.len() < want_samples {
            match self.format.next_packet() {
                Ok(Some(packet)) => {
                    if packet.track_id != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            let decoded_layout =
                                channel_layout_from_symphonia(audio_buf.spec().channels())?;
                            if audio_buf.spec().rate() != self.spec.sample_rate
                                || decoded_layout != self.spec.channel_layout
                            {
                                return Err(format!(
                                    "decoded PCM format changed after open: expected {:?}, got {}Hz {:?}",
                                    self.spec,
                                    audio_buf.spec().rate(),
                                    decoded_layout,
                                ));
                            }
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

fn channel_layout_from_symphonia(channels: &Channels) -> Result<ChannelLayout, String> {
    let Channels::Positioned(positions) = channels else {
        return Err(format!(
            "unsupported non-positioned channel layout: {channels}"
        ));
    };
    let mappings = [
        (Position::FRONT_LEFT, SpeakerPosition::FrontLeft),
        (Position::FRONT_RIGHT, SpeakerPosition::FrontRight),
        (Position::FRONT_CENTER, SpeakerPosition::FrontCenter),
        (Position::LFE1, SpeakerPosition::Lfe),
        (Position::REAR_LEFT, SpeakerPosition::RearLeft),
        (Position::REAR_RIGHT, SpeakerPosition::RearRight),
        (
            Position::FRONT_LEFT_CENTER,
            SpeakerPosition::FrontLeftCenter,
        ),
        (
            Position::FRONT_RIGHT_CENTER,
            SpeakerPosition::FrontRightCenter,
        ),
        (Position::REAR_CENTER, SpeakerPosition::RearCenter),
        (Position::SIDE_LEFT, SpeakerPosition::SideLeft),
        (Position::SIDE_RIGHT, SpeakerPosition::SideRight),
        (Position::TOP_CENTER, SpeakerPosition::TopCenter),
        (Position::TOP_FRONT_LEFT, SpeakerPosition::TopFrontLeft),
        (Position::TOP_FRONT_CENTER, SpeakerPosition::TopFrontCenter),
        (Position::TOP_FRONT_RIGHT, SpeakerPosition::TopFrontRight),
        (Position::TOP_REAR_LEFT, SpeakerPosition::TopRearLeft),
        (Position::TOP_REAR_CENTER, SpeakerPosition::TopRearCenter),
        (Position::TOP_REAR_RIGHT, SpeakerPosition::TopRearRight),
    ];
    let mapped = mappings
        .into_iter()
        .filter_map(|(symphonia, core)| positions.contains(symphonia).then_some(core))
        .collect::<Vec<_>>();
    if mapped.len() != positions.bits().count_ones() as usize {
        return Err(format!(
            "unsupported positioned channel outside the 7.1.4 speaker domain: {channels}"
        ));
    }
    ChannelLayout::from_positions(mapped)
        .map_err(|error| format!("unsupported positioned channel layout {channels}: {error}"))
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
    use std::io::Cursor;

    use stellatune_audio_core::format::{ChannelLayout, SpeakerPosition};
    use symphonia::core::audio::Channels;

    use super::{BuiltinDecoder, channel_layout_from_symphonia, extension_from_path};

    #[test]
    fn extension_from_path_understands_urls() {
        assert_eq!(
            extension_from_path("https://example.com/music/Track.FLAC?token=abc#frag"),
            "flac"
        );
        assert_eq!(extension_from_path("http://example.com/stream"), "");
        assert_eq!(extension_from_path("C:/music/song.mp3"), "mp3");
    }

    #[test]
    fn rejects_non_positioned_symphonia_channels() {
        assert!(channel_layout_from_symphonia(&Channels::Discrete(6)).is_err());
        assert!(channel_layout_from_symphonia(&Channels::Ambisonic(1)).is_err());
    }

    #[test]
    fn waveformat_extensible_5_1_preserves_layout_and_interleaved_order() {
        let bytes = pcm_5_1_side_wave();
        let mut decoder = BuiltinDecoder::open_source(Box::new(Cursor::new(bytes)), "wav").unwrap();

        assert_eq!(decoder.spec().sample_rate, 48_000);
        assert_eq!(
            decoder.spec().channel_layout,
            ChannelLayout::SURROUND_5_1_SIDE
        );
        assert_eq!(
            decoder
                .spec()
                .channel_layout
                .positions()
                .collect::<Vec<_>>(),
            vec![
                SpeakerPosition::FrontLeft,
                SpeakerPosition::FrontRight,
                SpeakerPosition::FrontCenter,
                SpeakerPosition::Lfe,
                SpeakerPosition::SideLeft,
                SpeakerPosition::SideRight,
            ]
        );

        let samples = decoder.next_block(1).unwrap().unwrap();
        assert_eq!(samples.len(), 6);
        for (sample, expected) in samples.iter().zip([1_i16, 2, 3, 4, 5, 6]) {
            let expected = f32::from(expected) / 32_768.0;
            assert!((sample - expected).abs() < 1.0e-6);
        }
    }

    fn pcm_5_1_side_wave() -> Vec<u8> {
        let channels = 6_u16;
        let sample_rate = 48_000_u32;
        let bits_per_sample = 16_u16;
        let block_align = channels * (bits_per_sample / 8);
        let byte_rate = sample_rate * u32::from(block_align);
        let samples = [1_i16, 2, 3, 4, 5, 6];
        let data_size = (samples.len() * 2) as u32;
        let riff_size = 4 + (8 + 40) + (8 + data_size);
        let mut bytes = Vec::with_capacity((riff_size + 8) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_size.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&40_u32.to_le_bytes());
        bytes.extend_from_slice(&0xfffe_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(&22_u16.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(&0x060f_u32.to_le_bytes());
        bytes.extend_from_slice(&[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38,
            0x9b, 0x71,
        ]);
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }
}
