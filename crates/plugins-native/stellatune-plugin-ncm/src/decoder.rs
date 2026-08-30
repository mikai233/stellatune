use std::io::{Read, Seek, SeekFrom};

use stellatune_plugin_sdk::prelude::*;
use symphonia::core::codecs::audio::{
    AudioDecoder as SymphoniaDecoder, AudioDecoderOptions as DecoderOptions,
};
use symphonia::core::common::Limit;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, StandardTag, StandardVisualKey};
use symphonia::core::units::Time;

use crate::flac_offset::find_flac_streaminfo_start;
use crate::io::{DecoderInputReader, NcmMediaSource};

const NCM_MAGIC: &[u8; 8] = b"CTENFDAM";

pub struct NcmWasmDecoderSession {
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
    cover: Option<Vec<u8>>,
}

impl ConfigStateOps for NcmWasmDecoderSession {}

impl DecoderSession for NcmWasmDecoderSession {
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

impl NcmWasmDecoderSession {
    pub fn open(input: DecoderInput) -> SdkResult<Self> {
        let DecoderInput { stream, ext_hint } = input;
        let ext_hint = ext_hint
            .as_deref()
            .map(|v| v.trim().to_ascii_lowercase())
            .filter(|v| !v.is_empty());

        let mut input_reader = DecoderInputReader::new(stream);
        let magic_ok = has_ncm_magic(&mut input_reader)?;
        let ext_ok = ext_hint.as_deref() == Some("ncm");
        if !magic_ok && !ext_ok {
            return Err(SdkError::unsupported(
                "input is neither .ncm extension nor ncm magic header",
            ));
        }

        input_reader.seek(SeekFrom::Start(0)).map_err(|e| {
            SdkError::internal(format!(
                "failed to reset input stream before ncmdump parse: {e}"
            ))
        })?;

        let mut ncm = ncmdump::Ncmdump::from_reader(input_reader)
            .map_err(|e| SdkError::internal(format!("ncmdump parse failed: {e}")))?;

        let info = ncm
            .get_info()
            .map_err(|e| SdkError::internal(format!("ncmdump get_info failed: {e}")))?;
        let cover = ncm.get_image().ok().filter(|v| !v.is_empty());
        let mut tags = Tags {
            title: Some(info.name.trim().to_string()).filter(|v| !v.is_empty()),
            album: Some(info.album.trim().to_string()).filter(|v| !v.is_empty()),
            artists: info
                .artist
                .iter()
                .map(|(name, _id)| name.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect(),
            cover,
        };

        let payload_start = ncm
            .stream_position()
            .map_err(|e| SdkError::internal(format!("ncmdump tell failed: {e}")))?;
        let payload_end = ncm
            .seek(SeekFrom::End(0))
            .map_err(|e| SdkError::internal(format!("ncmdump seek end failed: {e}")))?;
        ncm.seek(SeekFrom::Start(0))
            .map_err(|e| SdkError::internal(format!("ncmdump seek start failed: {e}")))?;

        let hint_ext = info.format.trim().to_ascii_lowercase();
        let mut hint = Hint::new();
        if !hint_ext.is_empty() {
            hint.with_extension(&hint_ext);
        }

        let mut start_offset = 0u64;
        if hint_ext.eq_ignore_ascii_case("flac") {
            start_offset = find_flac_streaminfo_start(&mut ncm)
                .map_err(|e| SdkError::internal(format!("flac offset scan failed: {e}")))?;
            ncm.seek(SeekFrom::Start(start_offset)).map_err(|e| {
                SdkError::internal(format!("ncmdump seek to flac start failed: {e}"))
            })?;
        }

        let payload_len = payload_end.saturating_sub(payload_start);
        let len = payload_len.saturating_sub(start_offset);
        let src = NcmMediaSource::new(ncm, start_offset, Some(len));
        let mss = MediaSourceStream::new(Box::new(src), MediaSourceStreamOptions::default());

        let meta_opts =
            MetadataOptions::default().limit_visual_bytes(Limit::Maximum(12 * 1024 * 1024));
        let mut format = symphonia::default::get_probe()
            .probe(&hint, mss, FormatOptions::default(), meta_opts)
            .map_err(|e| SdkError::internal(format!("symphonia probe failed: {e}")))?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or_else(|| SdkError::internal("missing audio track"))?;
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
        let in_channels = params.channels.as_ref().map(|c| c.count()).unwrap_or(0);
        if in_channels == 0 {
            return Err(SdkError::internal("missing channels"));
        }
        let out_channels: u16 = if in_channels == 1 { 1 } else { 2 };

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &DecoderOptions::default())
            .map_err(|e| SdkError::internal(format!("decoder init failed: {e}")))?;

        let duration_ms = Some(info.duration).filter(|v| *v > 0).or_else(|| {
            time_base.and_then(|tb| {
                num_frames.and_then(|n| {
                    tb.calc_time(symphonia::core::units::Timestamp::new(
                        n.min(i64::MAX as u64) as i64,
                    ))
                    .map(|time| (time.as_secs_f64() * 1000.0).round() as u64)
                })
            })
        });

        {
            let mut m = format.metadata();
            if let Some(rev) = m.skip_to_latest() {
                apply_revision(rev, &mut tags);
            }
        }

        let metadata = build_metadata(
            tags,
            duration_ms,
            sample_rate,
            out_channels,
            if hint_ext.is_empty() {
                "ncm".to_string()
            } else {
                hint_ext
            },
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
                seekable: true,
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
        let want = (frames as usize).saturating_mul(self.out_channels.max(1) as usize);
        if out_interleaved.len() < want {
            return Err(SdkError::invalid_arg("output buffer too small"));
        }

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

                            let in_ch = self.in_channels.max(1);
                            let out_ch = self.out_channels.max(1) as usize;
                            let frames = samples.len() / in_ch;

                            if in_ch == out_ch {
                                self.pending.extend_from_slice(&samples);
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
                        Err(e) => return Err(SdkError::internal(format!("decode failed: {e}"))),
                    }
                },
                Ok(None) => break,
                Err(SymphoniaError::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
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
        Ok(((take / self.out_channels.max(1) as usize) as u32, false))
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
            Some(StandardTag::Artist(value)) if tags.artists.is_empty() => {
                if let Some(artist) = normalize_tag_text(value.as_str()) {
                    tags.artists.push(artist);
                }
            },
            _ => {},
        }
    }

    if tags.cover.is_none() {
        let visuals = rev
            .media
            .visuals
            .iter()
            .chain(
                rev.per_track
                    .iter()
                    .flat_map(|track| track.metadata.visuals.iter()),
            )
            .collect::<Vec<_>>();
        let front = visuals
            .iter()
            .copied()
            .find(|v| v.usage == Some(StandardVisualKey::FrontCover));
        let any = visuals.first().copied();
        let chosen = front.or(any);
        if let Some(bytes) = chosen.and_then(|v| (!v.data.is_empty()).then(|| v.data.to_vec())) {
            tags.cover = Some(bytes);
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
    let mut extras = Vec::<MetadataEntry>::new();
    if let Some(cover) = tags.cover {
        extras.push(MetadataEntry {
            key: "cover".to_string(),
            value: MetadataValue::Bytes(cover),
        });
    }
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
            container: Some("ncm".to_string()),
        },
        extras,
    }
}

fn has_ncm_magic<R>(reader: &mut R) -> SdkResult<bool>
where
    R: Read + Seek,
{
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|e| SdkError::internal(format!("failed to seek input to start: {e}")))?;

    let mut magic = [0u8; NCM_MAGIC.len()];
    let read_result = reader.read_exact(&mut magic);

    reader
        .seek(SeekFrom::Start(0))
        .map_err(|e| SdkError::internal(format!("failed to restore input position: {e}")))?;

    match read_result {
        Ok(()) => Ok(&magic == NCM_MAGIC),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(SdkError::internal(format!(
            "failed to read ncm magic header: {error}"
        ))),
    }
}
