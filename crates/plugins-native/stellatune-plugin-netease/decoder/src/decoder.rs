use std::io::{Seek, SeekFrom};

use stellatune_plugin_sdk::prelude::*;
use symphonia::core::codecs::audio::{
    AudioDecoder as SymphoniaDecoder, AudioDecoderOptions as DecoderOptions,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, StandardTag};
use symphonia::core::units::Time;

use crate::io::{DecoderInputReader, StreamMediaSource};

pub struct StreamSymphoniaDecoderSession {
    backend: SymphoniaBackend,
}

struct SymphoniaBackend {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn SymphoniaDecoder>,
    track_id: u32,
    in_channels: usize,
    sample_rate: u32,
    out_channels: u16,
    pending: Vec<f32>,
    metadata: MediaMetadata,
    duration_ms: Option<u64>,
    seekable: bool,
    encoder_delay_frames: u32,
    encoder_padding_frames: u32,
}

#[derive(Default)]
struct Tags {
    title: Option<String>,
    album: Option<String>,
    artists: Vec<String>,
}

impl ConfigStateOps for StreamSymphoniaDecoderSession {}

impl DecoderSession for StreamSymphoniaDecoderSession {
    fn info(&self) -> SdkResult<DecoderInfo> {
        Ok(DecoderInfo {
            sample_rate: self.backend.sample_rate,
            channels: self.backend.out_channels,
            duration_ms: self.backend.duration_ms,
            seekable: self.backend.seekable,
            encoder_delay_frames: self.backend.encoder_delay_frames,
            encoder_padding_frames: self.backend.encoder_padding_frames,
        })
    }

    fn metadata(&self) -> SdkResult<MediaMetadata> {
        Ok(self.backend.metadata.clone())
    }

    fn read_pcm_f32(&mut self, max_frames: u32) -> SdkResult<PcmF32Chunk> {
        if max_frames == 0 {
            return Ok(PcmF32Chunk {
                interleaved_f32le: Vec::new(),
                frames: 0,
                eof: false,
            });
        }

        let mut out_samples =
            vec![0.0f32; max_frames as usize * self.backend.out_channels.max(1) as usize];
        let (frames, eof) = self
            .backend
            .read_interleaved_f32(max_frames, out_samples.as_mut_slice())?;

        let used = frames as usize * self.backend.out_channels.max(1) as usize;
        let mut bytes = Vec::<u8>::with_capacity(used * std::mem::size_of::<f32>());
        for sample in &out_samples[..used] {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        Ok(PcmF32Chunk {
            interleaved_f32le: bytes,
            frames,
            eof,
        })
    }

    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()> {
        self.backend.seek_ms(position_ms)
    }
}

impl StreamSymphoniaDecoderSession {
    pub fn open(input: DecoderInput) -> SdkResult<Self> {
        let DecoderInput { stream, ext_hint } = input;
        let ext_hint = ext_hint
            .as_deref()
            .map(|v| v.trim().to_ascii_lowercase())
            .filter(|v| !v.is_empty());

        let mut reader = DecoderInputReader::new(stream);
        let byte_len = reader.size().ok();
        let seekable = reader.stream_position().is_ok();
        if seekable {
            let _ = reader.seek(SeekFrom::Start(0));
        }

        let mut hint = Hint::new();
        if let Some(ext) = ext_hint.as_deref() {
            hint.with_extension(ext);
        }

        let src = StreamMediaSource::new(reader, byte_len, seekable);
        let mss = MediaSourceStream::new(Box::new(src), MediaSourceStreamOptions::default());

        let mut format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| SdkError::internal(format!("symphonia probe failed: {e}")))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| SdkError::internal("missing default audio track"))?;
        let track_id = track.id;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .cloned()
            .ok_or_else(|| SdkError::internal("missing audio codec parameters"))?;
        let time_base = track.time_base;
        let num_frames = track.num_frames;
        let encoder_delay_frames = track.delay.unwrap_or(0);
        let encoder_padding_frames = track.padding.unwrap_or(0);

        let sample_rate = params
            .sample_rate
            .ok_or_else(|| SdkError::internal("missing sample rate"))?;
        let in_channels = params.channels.as_ref().map(|v| v.count()).unwrap_or(0);
        if in_channels == 0 {
            return Err(SdkError::internal("missing channels"));
        }
        let out_channels: u16 = if in_channels == 1 { 1 } else { 2 };

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &DecoderOptions::default())
            .map_err(|e| SdkError::internal(format!("decoder init failed: {e}")))?;

        let duration_ms = time_base.and_then(|tb| {
            num_frames.and_then(|n| {
                tb.calc_time(symphonia::core::units::Timestamp::new(
                    n.min(i64::MAX as u64) as i64,
                ))
                .map(|time| (time.as_secs_f64() * 1000.0).round() as u64)
            })
        });

        let mut tags = Tags::default();
        {
            let mut metadata = format.metadata();
            if let Some(rev) = metadata.skip_to_latest() {
                apply_revision(rev, &mut tags);
            }
        }

        let metadata = build_metadata(
            tags,
            duration_ms,
            sample_rate,
            out_channels,
            ext_hint.unwrap_or_else(|| "stream".to_string()),
        );

        Ok(Self {
            backend: SymphoniaBackend {
                format,
                decoder,
                track_id,
                in_channels,
                sample_rate,
                out_channels,
                pending: Vec::new(),
                metadata,
                duration_ms,
                seekable,
                encoder_delay_frames,
                encoder_padding_frames,
            },
        })
    }
}

impl SymphoniaBackend {
    fn seek_ms(&mut self, position_ms: u64) -> SdkResult<()> {
        if !self.seekable {
            return Err(SdkError::unsupported("seek not supported"));
        }

        let time = Time::from_millis_u64(position_ms);
        let _ = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| SdkError::internal(format!("seek failed: {e}")))?;

        self.decoder.reset();
        self.pending.clear();
        Ok(())
    }

    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)> {
        let out_channels = self.out_channels.max(1) as usize;
        let want = (frames as usize).saturating_mul(out_channels);
        if out_interleaved.len() < want {
            return Err(SdkError::invalid_arg("output buffer too small"));
        }

        let mut reached_eof = false;
        while self.pending.len() < want {
            match self.format.next_packet() {
                Ok(Some(packet)) => {
                    if packet.track_id != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(audio_buf) => {
                            let mut samples = vec![0.0f32; audio_buf.samples_interleaved()];
                            audio_buf.copy_to_slice_interleaved(&mut samples);

                            push_pending_samples(
                                &mut self.pending,
                                &samples,
                                self.in_channels,
                                out_channels,
                            );
                        },
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::ResetRequired) => {
                            self.decoder.reset();
                            continue;
                        },
                        Err(e) => return Err(SdkError::internal(format!("decode failed: {e}"))),
                    }
                },
                Ok(None) => {
                    reached_eof = true;
                    break;
                },
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    reached_eof = true;
                    break;
                },
                Err(e) => return Err(SdkError::internal(format!("read packet failed: {e}"))),
            }
        }

        if self.pending.is_empty() {
            return Ok((0, true));
        }

        let take = want.min(self.pending.len());
        out_interleaved[..take].copy_from_slice(&self.pending[..take]);
        self.pending.drain(..take);
        Ok((
            (take / out_channels) as u32,
            reached_eof && self.pending.is_empty(),
        ))
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
            let left = samples[i * in_channels];
            let right = if in_channels > 1 {
                samples[i * in_channels + 1]
            } else {
                left
            };
            out.push(((left + right) * 0.5).clamp(-1.0, 1.0));
        }
        return;
    }

    let extra = in_channels.saturating_sub(2) as f32;
    let norm = 1.0 / (1.0 + 0.5 * extra).max(1.0);
    for i in 0..frames {
        let base = i * in_channels;
        let mut left = samples[base];
        let mut right = if in_channels > 1 {
            samples[base + 1]
        } else {
            left
        };

        for ch in 2..in_channels {
            let v = samples[base + ch];
            left += 0.5 * v;
            right += 0.5 * v;
        }

        out.push((left * norm).clamp(-1.0, 1.0));
        out.push((right * norm).clamp(-1.0, 1.0));
    }
}

fn apply_revision(rev: &symphonia::core::meta::MetadataRevision, tags: &mut Tags) {
    for tag in rev.media.tags.iter().chain(
        rev.per_track
            .iter()
            .flat_map(|track| track.metadata.tags.iter()),
    ) {
        match tag.std.as_ref() {
            Some(StandardTag::TrackTitle(value)) if tags.title.is_none() => {
                tags.title = normalize_tag_text(value.as_str());
            },
            Some(StandardTag::Album(value)) if tags.album.is_none() => {
                tags.album = normalize_tag_text(value.as_str());
            },
            Some(StandardTag::Artist(value)) => {
                if let Some(value) = normalize_tag_text(value.as_str())
                    && !tags.artists.iter().any(|artist| artist == &value)
                {
                    tags.artists.push(value);
                }
            },
            _ => {},
        }
    }
}

fn normalize_tag_text(value: &str) -> Option<String> {
    let s = value.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn build_metadata(
    tags: Tags,
    duration_ms: Option<u64>,
    sample_rate: u32,
    channels: u16,
    codec: String,
) -> MediaMetadata {
    MediaMetadata {
        tags: AudioTags {
            title: tags.title,
            album: tags.album,
            artists: tags.artists,
            ..AudioTags::default()
        },
        duration_ms,
        format: EncodedAudioFormat {
            codec,
            sample_rate: Some(sample_rate),
            channels: Some(channels),
            bitrate_kbps: None,
            container: None,
        },
        extras: Vec::new(),
    }
}
