//! Bounded, seekable virtual WAV resources backed by the shared segment decoder.
use anyhow::{Context, Result, bail};
use std::sync::Arc;
use std::time::{Duration, Instant};
use stellatune_audio_builtin_adapters::factories::{FileSourceFactory, SymphoniaDecoderFactory};
use stellatune_audio_core::{
    decoder::{DecodeStatus, DecoderFactory, DecoderSeekStatus, DecoderStage},
    format::{AudioBlock, PcmFormat},
    segment::SegmentDecoder,
    source::{SourceCancellation, SourceFactory, SourceOpenPurpose, SourceOpenRequest},
};
use stellatune_library::catalog::LocalTrackResource;

#[derive(Clone)]
pub struct SegmentWave {
    pub resource: LocalTrackResource,
    pub format: PcmFormat,
    pub header: Vec<u8>,
    pub length: u64,
    bits: u16,
    float: bool,
}

async fn decoder(
    resource: &LocalTrackResource,
    cancellation: SourceCancellation,
) -> Result<(Box<dyn DecoderStage>, PcmFormat)> {
    let segment = resource
        .segment
        .context("WAV segment resource has no interval")?;
    let factory = FileSourceFactory::new(resource.path.clone().into(), Default::default())?;
    let hints = factory.descriptor().media;
    let source = factory
        .open(SourceOpenRequest {
            purpose: SourceOpenPurpose::Initial,
            deadline: Some(Instant::now() + Duration::from_secs(30)),
            cancellation,
        })
        .await?;
    tokio::task::spawn_blocking(move || {
        let inner = SymphoniaDecoderFactory::new().create()?;
        let mut decoder = SegmentDecoder::new(inner, segment);
        let info = decoder.open(source, &hints)?;
        Ok((Box::new(decoder) as Box<dyn DecoderStage>, info.format))
    })
    .await?
}

impl SegmentWave {
    pub async fn prepare(resource: LocalTrackResource) -> Result<Self> {
        let bits = resource
            .pcm_bits
            .context("source PCM precision is unknown; cannot publish without reducing quality")?;
        let float = resource.pcm_float;
        if !matches!((float, bits), (false, 16 | 24) | (true, 32)) {
            bail!(
                "source PCM precision cannot be retained by this decoder ({} bits)",
                bits
            );
        }
        let (_, format) = decoder(&resource, SourceCancellation::default()).await?;
        let segment = resource.segment.context("missing interval")?;
        let frames = segment.end_frame_exclusive - segment.start_frame;
        let data_len = u128::from(frames)
            * u128::from(format.channel_layout.channel_count())
            * u128::from(bits / 8);
        let header = wave_header(
            format,
            bits as u16,
            float,
            frames,
            u32::try_from(data_len).context("segment exceeds standard WAV size limit")?,
        )?;
        let length = header.len() as u64 + data_len as u64 + (data_len as u64 % 2);
        Ok(Self {
            resource,
            format,
            header,
            length,
            bits: bits as u16,
            float,
        })
    }

    /// Every range owns its decoder; no complete audio file is retained in RAM.
    pub async fn stream(
        &self,
        start: u64,
        end: u64,
        cancellation: SourceCancellation,
    ) -> Result<tokio::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>> {
        static PERMITS: std::sync::OnceLock<Arc<tokio::sync::Semaphore>> =
            std::sync::OnceLock::new();
        let permits = PERMITS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)));
        let permit = permits
            .clone()
            .try_acquire_owned()
            .context("too many concurrent DLNA audio requests")?;
        if start > end || end >= self.length {
            bail!("invalid WAV byte range");
        }
        let (mut decoder, _) = decoder(&self.resource, cancellation.clone()).await?;
        let wave = self.clone();
        let (sender, receiver) = tokio::sync::mpsc::channel(2);
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let result: Result<()> = (|| {
                let header_len = wave.header.len() as u64;
                let mut remaining = end - start + 1;
                if start < header_len {
                    let n = remaining.min(header_len - start);
                    if sender
                        .blocking_send(Ok(
                            wave.header[start as usize..(start + n) as usize].to_vec()
                        ))
                        .is_err()
                    {
                        return Ok(());
                    }
                    remaining -= n;
                }
                if remaining == 0 {
                    return Ok(());
                }
                let alignment = u64::from(wave.bits / 8)
                    * u64::from(wave.format.channel_layout.channel_count());
                let data_start = start.saturating_sub(header_len);
                let data_length = u64::from(u32::from_le_bytes(
                    wave.header[wave.header.len() - 4..].try_into().unwrap(),
                ));
                let padding = (end >= header_len + data_length) as u64;
                if data_start >= data_length {
                    let _ = sender.blocking_send(Ok(vec![0; remaining as usize]));
                    return Ok(());
                }
                remaining -= padding;
                let mut skip_bytes = (data_start % alignment) as usize;
                let mut seek = decoder.start_seek(data_start / alignment)?;
                let mut deadline = Instant::now() + Duration::from_secs(30);
                while matches!(seek, DecoderSeekStatus::Pending) {
                    if sender.is_closed() || cancellation.is_cancelled() {
                        return Ok(());
                    }
                    if Instant::now() > deadline {
                        bail!("DLNA segment seek timed out");
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    seek = decoder.continue_seek()?;
                }
                while remaining > 0 && !sender.is_closed() && !cancellation.is_cancelled() {
                    let mut block = AudioBlock::new(wave.format);
                    match decoder.decode(&mut block)? {
                        DecodeStatus::Pending => {
                            if Instant::now() > deadline {
                                bail!("DLNA segment decoding timed out");
                            }
                            std::thread::sleep(Duration::from_millis(1));
                        },
                        DecodeStatus::EndOfStream => bail!("unexpected EOF in WAV byte range"),
                        DecodeStatus::Produced { .. } => {
                            deadline = Instant::now() + Duration::from_secs(30);
                            let mut bytes = Vec::with_capacity(
                                block.samples.len() * usize::from(wave.bits / 8),
                            );
                            for sample in block.samples {
                                match (wave.float, wave.bits) {
                                    (true, 32) => bytes.extend(sample.to_le_bytes()),
                                    (false, 16) => bytes.extend(
                                        ((sample * 32768.0).round().clamp(-32768.0, 32767.0)
                                            as i16)
                                            .to_le_bytes(),
                                    ),
                                    (false, 24) => bytes.extend_from_slice(
                                        &((sample * 8388608.0).round().clamp(-8388608.0, 8388607.0)
                                            as i32)
                                            .to_le_bytes()[..3],
                                    ),
                                    _ => unreachable!(),
                                }
                            }
                            let skip = skip_bytes.min(bytes.len());
                            bytes.drain(..skip);
                            skip_bytes -= skip;
                            bytes.truncate(remaining.min(bytes.len() as u64) as usize);
                            remaining -= bytes.len() as u64;
                            if !bytes.is_empty() && sender.blocking_send(Ok(bytes)).is_err() {
                                return Ok(());
                            }
                        },
                    }
                }
                if padding != 0 && !cancellation.is_cancelled() {
                    let _ = sender.blocking_send(Ok(vec![0]));
                }
                Ok(())
            })();
            if let Err(error) = result {
                let _ = sender.blocking_send(Err(std::io::Error::other(error.to_string())));
            }
        });
        Ok(receiver)
    }
}

fn wave_header(
    format: PcmFormat,
    bits: u16,
    float: bool,
    frames: u64,
    data_len: u32,
) -> Result<Vec<u8>> {
    let channels = format.channel_layout.channel_count();
    let extended = channels > 2;
    let fmt_len = if extended {
        40_u32
    } else if float {
        18
    } else {
        16
    };
    let fact_len = if float { 12 } else { 0 };
    let riff_len = 4_u32
        .checked_add(8 + fmt_len + fact_len + 8)
        .and_then(|v| v.checked_add(data_len))
        .and_then(|v| v.checked_add(data_len % 2))
        .context("segment exceeds standard WAV size limit")?;
    let align = channels
        .checked_mul(bits / 8)
        .context("PCM block alignment overflow")?;
    let byte_rate = format
        .sample_rate
        .checked_mul(u32::from(align))
        .context("PCM byte rate overflow")?;
    let tag = if float { 3_u16 } else { 1 };
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend(riff_len.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(fmt_len.to_le_bytes());
    bytes.extend((if extended { 0xfffe_u16 } else { tag }).to_le_bytes());
    bytes.extend(channels.to_le_bytes());
    bytes.extend(format.sample_rate.to_le_bytes());
    bytes.extend(byte_rate.to_le_bytes());
    bytes.extend(align.to_le_bytes());
    bytes.extend(bits.to_le_bytes());
    if extended {
        bytes.extend(22_u16.to_le_bytes());
        bytes.extend(bits.to_le_bytes());
        let mask = format
            .channel_layout
            .positions()
            .fold(0_u32, |mask, position| mask | (1 << position as u32));
        bytes.extend(mask.to_le_bytes());
        bytes.extend(u32::from(tag).to_le_bytes());
        bytes.extend([0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71]);
    } else if float {
        bytes.extend(0_u16.to_le_bytes());
    }
    if float {
        bytes.extend(b"fact");
        bytes.extend(4_u32.to_le_bytes());
        bytes.extend(u32::try_from(frames)?.to_le_bytes());
    }
    bytes.extend(b"data");
    bytes.extend(data_len.to_le_bytes());
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellatune_audio_core::{format::ChannelLayout, segment::AudioSegment};

    async fn bytes(wave: &SegmentWave, start: u64, end: u64) -> Vec<u8> {
        let mut stream = wave
            .stream(start, end, SourceCancellation::default())
            .await
            .unwrap();
        let mut result = Vec::new();
        while let Some(block) = stream.recv().await {
            result.extend(block.unwrap());
        }
        result
    }

    #[tokio::test]
    #[ignore = "Read-only acceptance on a real CUE album"]
    async fn real_album_segments_read_only() {
        let root =
            std::env::var("STELLATUNE_CUE_ALBUM_ROOT").expect("set STELLATUNE_CUE_ALBUM_ROOT");
        let dir = tempfile::tempdir().unwrap();
        let library = stellatune_library::start_library(
            dir.path()
                .join("acceptance.sqlite")
                .to_string_lossy()
                .into_owned(),
        )
        .await
        .unwrap();
        let mut events = library.subscribe_events();
        library.add_root(root.clone()).await.unwrap();
        library.scan_all().await.unwrap();
        let tracks = library
            .list_tracks(String::new(), true, String::new(), 200, 0)
            .await
            .unwrap();
        while let Ok(event) = events.try_recv() {
            eprintln!("{event:?}");
        }
        let segments = tracks.iter().filter(|t| t.is_segment).collect::<Vec<_>>();
        assert!(!segments.is_empty());
        for index in [0, segments.len() / 2, segments.len() - 1] {
            let track = segments[index];
            let resource = library.catalog().playback_resource(track.id).await.unwrap();
            let wave = SegmentWave::prepare(resource).await.unwrap();
            let start = wave.header.len() as u64;
            let end = (start + 10000).min(wave.length - 1);
            let full = bytes(&wave, start, end).await;
            let mut split = bytes(&wave, start, start + 997).await;
            split.extend(bytes(&wave, start + 998, end).await);
            assert_eq!(full, split);
            eprintln!(
                "Accepted {:?}: {:?}, {} Hz, {}-bit PCM",
                track.title, wave.resource.segment, wave.format.sample_rate, wave.bits
            );
        }
        library.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn virtual_wave_preserves_pcm_and_unaligned_ranges() {
        let dir = tempfile::tempdir().unwrap();
        for (bits, float) in [(16, false), (24, false), (32, true)] {
            let format = PcmFormat {
                sample_rate: 48000,
                channel_layout: ChannelLayout::MONO,
            };
            let samples: Vec<u8> = (0..3001_i32)
                .flat_map(|n| {
                    let n = (n * 511) % 65536 - 32768;
                    if float {
                        (n as f32 / 32768.0).to_le_bytes().to_vec()
                    } else if bits == 16 {
                        (n as i16).to_le_bytes().to_vec()
                    } else {
                        (n * 255).to_le_bytes()[..3].to_vec()
                    }
                })
                .collect();
            let mut original =
                wave_header(format, bits, float, 3001, samples.len() as u32).unwrap();
            original.extend(&samples);
            if samples.len() % 2 != 0 {
                original.push(0);
            }
            let path = dir.path().join(format!("{bits}.wav"));
            std::fs::write(&path, original).unwrap();
            let wave = SegmentWave::prepare(LocalTrackResource {
                path: path.to_string_lossy().into_owned(),
                segment: Some(AudioSegment {
                    start_frame: 97,
                    end_frame_exclusive: 2900,
                    sample_rate: 48000,
                }),
                cover_key: 1,
                pcm_bits: Some(bits as u32),
                pcm_float: float,
            })
            .await
            .unwrap();
            let full = bytes(&wave, 0, wave.length - 1).await;
            assert_eq!(full.len() as u64, wave.length);
            assert_eq!(
                u32::from_le_bytes(full[4..8].try_into().unwrap()) as usize + 8,
                full.len()
            );
            let width = bits as usize / 8;
            let data_len = 2803 * width;
            assert_eq!(
                &full[wave.header.len()..wave.header.len() + data_len],
                &samples[97 * width..2900 * width],
                "{bits}-bit PCM changed"
            );
            let mut assembled = Vec::new();
            for start in (0..wave.length).step_by(719) {
                assembled.extend(bytes(&wave, start, (start + 718).min(wave.length - 1)).await);
            }
            assert_eq!(assembled, full);
            assert_eq!(
                bytes(&wave, wave.length - 1, wave.length - 1).await,
                full[full.len() - 1..]
            );
            let cancellation = SourceCancellation::default();
            let receiver = wave
                .stream(0, wave.length - 1, cancellation.clone())
                .await
                .unwrap();
            cancellation.cancel();
            drop(receiver);
        }
    }
}
