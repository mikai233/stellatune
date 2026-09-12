//! A decoder view whose entire public timeline belongs to one audio segment.
use crate::decoder::{
    DecodeStatus, DecodedStreamInfo, DecoderSeekStatus, DecoderStage, SeekResult,
};
use crate::error::DecodeError;
use crate::format::AudioBlock;
use crate::source::{EncodedSource, MediaHints};

/// An exclusive range on the source's audible (encoder padding removed) timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSegment {
    /// Inclusive first audible sample frame.
    pub start_frame: u64,
    /// Exclusive last audible sample frame.
    pub end_frame_exclusive: u64,
    /// Source sampling rate; mismatches invalidate a stale segment.
    pub sample_rate: u32,
}

/// Wraps any decoder; no filesystem, CUE, or playback policy is involved.
pub struct SegmentDecoder {
    inner: Box<dyn DecoderStage>,
    segment: AudioSegment,
    head: u64,
    cursor: u64,
    target: u64,
    pending: bool,
}

impl SegmentDecoder {
    /// Construct an unopened segment view.
    pub fn new(inner: Box<dyn DecoderStage>, segment: AudioSegment) -> Self {
        Self {
            inner,
            segment,
            head: 0,
            cursor: 0,
            target: 0,
            pending: false,
        }
    }

    fn start(&self) -> u64 {
        self.segment.start_frame + self.head
    }
    fn end(&self) -> u64 {
        self.segment.end_frame_exclusive + self.head
    }
    fn length(&self) -> u64 {
        self.segment.end_frame_exclusive - self.segment.start_frame
    }
    fn finish(&mut self, status: DecoderSeekStatus) -> Result<DecoderSeekStatus, DecodeError> {
        match status {
            DecoderSeekStatus::Pending => {
                self.pending = true;
                Ok(status)
            },
            DecoderSeekStatus::Complete(result) => {
                if result.actual_frame > self.target {
                    return Err(DecodeError::Failed {
                        message: "decoder seek overshot segment target".into(),
                    });
                }
                self.pending = false;
                self.cursor = result.actual_frame;
                // decode() discards any coarse-seek prefix before publishing PCM.
                Ok(DecoderSeekStatus::Complete(SeekResult {
                    actual_frame: self.target - self.start(),
                }))
            },
        }
    }
}

impl DecoderStage for SegmentDecoder {
    fn configure_buffering(&mut self, config: crate::buffering::BufferingConfig) {
        self.inner.configure_buffering(config);
    }
    fn set_waker(&mut self, waker: std::task::Waker) {
        self.inner.set_waker(waker);
    }
    fn open(
        &mut self,
        source: Box<dyn EncodedSource>,
        hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        let mut info = self.inner.open(source, hints)?;
        self.head = info
            .gapless_trim
            .map_or(0, |trim| u64::from(trim.head_frames));
        let tail = info
            .gapless_trim
            .map_or(0, |trim| u64::from(trim.tail_frames));
        let duration = info.duration_frames.ok_or(DecodeError::Unsupported)?;
        if self.segment.sample_rate == 0
            || self.segment.sample_rate != info.format.sample_rate
            || self.segment.start_frame >= self.segment.end_frame_exclusive
            || self.segment.end_frame_exclusive
                > duration.saturating_sub(self.head.saturating_add(tail))
        {
            return Err(DecodeError::Failed {
                message: "invalid or stale audio segment".into(),
            });
        }
        self.target = self.start();
        let status = self.inner.start_seek(self.target)?;
        self.finish(status)?;
        info.duration_frames = Some(self.length());
        info.gapless_trim = None;
        Ok(info)
    }
    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        output.samples.clear();
        if self.pending {
            let status = self.inner.continue_seek()?;
            if matches!(self.finish(status)?, DecoderSeekStatus::Pending) {
                return Ok(DecodeStatus::Pending);
            }
        }
        if self.cursor >= self.end() {
            return Ok(DecodeStatus::EndOfStream);
        }
        match self.inner.decode(output)? {
            DecodeStatus::Produced { frames } => {
                let channels = usize::from(output.format.channel_layout.channel_count());
                if frames != output.frames() {
                    return Err(DecodeError::Failed {
                        message: "misaligned decoder output".into(),
                    });
                }
                let start = self.cursor;
                self.cursor = self.cursor.saturating_add(frames as u64);
                let skip = self.target.saturating_sub(start).min(frames as u64) as usize;
                let keep_end = self.end().saturating_sub(start).min(frames as u64) as usize;
                if keep_end <= skip {
                    output.samples.clear();
                    return Ok(DecodeStatus::Pending);
                }
                output.samples.truncate(keep_end * channels);
                output.samples.drain(..skip * channels);
                output.timeline.start_frame = (start + skip as u64) - self.start();
                Ok(DecodeStatus::Produced {
                    frames: keep_end - skip,
                })
            },
            DecodeStatus::EndOfStream => Err(DecodeError::Failed {
                message: "audio source ended before segment boundary".into(),
            }),
            DecodeStatus::Pending => Ok(DecodeStatus::Pending),
        }
    }
    fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
        self.target = self.start() + target_frame.min(self.length());
        if self.target == self.end() {
            self.cursor = self.target;
            self.pending = false;
            return Ok(DecoderSeekStatus::Complete(SeekResult {
                actual_frame: self.length(),
            }));
        }
        let status = self.inner.start_seek(self.target)?;
        self.finish(status)
    }
    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        let status = self.inner.continue_seek()?;
        self.finish(status)
    }
    fn reset(&mut self) {
        self.inner.reset();
        self.cursor = 0;
        self.target = 0;
        self.pending = false;
        self.head = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::GaplessTrimSpec;
    use crate::format::{ChannelLayout, PcmFormat};
    use std::io::{Read, Seek, SeekFrom};
    struct EmptySource;
    impl Read for EmptySource {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Ok(0)
        }
    }
    impl Seek for EmptySource {
        fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
            Ok(0)
        }
    }
    impl EncodedSource for EmptySource {
        fn byte_len(&self) -> Option<u64> {
            Some(0)
        }
        fn is_seekable(&self) -> bool {
            true
        }
    }
    struct Samples {
        cursor: u64,
        target: u64,
    }
    fn format() -> PcmFormat {
        PcmFormat {
            sample_rate: 48000,
            channel_layout: ChannelLayout::MONO,
        }
    }
    impl DecoderStage for Samples {
        fn open(
            &mut self,
            _: Box<dyn EncodedSource>,
            _: &MediaHints,
        ) -> Result<DecodedStreamInfo, DecodeError> {
            Ok(DecodedStreamInfo {
                format: format(),
                duration_frames: Some(25),
                gapless_trim: Some(GaplessTrimSpec {
                    head_frames: 2,
                    tail_frames: 3,
                }),
            })
        }
        fn decode(&mut self, block: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
            if self.cursor >= 25 {
                return Ok(DecodeStatus::EndOfStream);
            }
            let end = (self.cursor + 4).min(25);
            block.format = format();
            block.samples = (self.cursor..end).map(|i| i as f32).collect();
            self.cursor = end;
            Ok(DecodeStatus::Produced {
                frames: block.frames(),
            })
        }
        fn start_seek(&mut self, target: u64) -> Result<DecoderSeekStatus, DecodeError> {
            self.target = target;
            Ok(DecoderSeekStatus::Pending)
        }
        fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
            self.cursor = self.target / 4 * 4;
            Ok(DecoderSeekStatus::Complete(SeekResult {
                actual_frame: self.cursor,
            }))
        }
        fn reset(&mut self) {
            self.cursor = 0;
        }
    }
    fn opened(start: u64, end: u64) -> SegmentDecoder {
        let mut segment = SegmentDecoder::new(
            Box::new(Samples {
                cursor: 0,
                target: 0,
            }),
            AudioSegment {
                start_frame: start,
                end_frame_exclusive: end,
                sample_rate: 48000,
            },
        );
        let info = segment
            .open(Box::new(EmptySource), &MediaHints::default())
            .unwrap();
        assert_eq!(info.duration_frames, Some(end - start));
        assert!(info.gapless_trim.is_none());
        segment
    }
    fn collect(decoder: &mut SegmentDecoder) -> Vec<f32> {
        let mut samples = Vec::new();
        for _ in 0..100 {
            let mut block = AudioBlock::new(format());
            match decoder.decode(&mut block).unwrap() {
                DecodeStatus::Produced { .. } => samples.extend(block.samples),
                DecodeStatus::Pending => {},
                DecodeStatus::EndOfStream => return samples,
            }
        }
        panic!("segment did not reach EOF")
    }
    #[test]
    fn clips_coarse_seeks_and_packet_ends_without_double_padding() {
        let mut segment = opened(5, 13);
        assert_eq!(
            collect(&mut segment),
            (7..15).map(|v| v as f32).collect::<Vec<_>>()
        );
        for target in [0, 1, 4, 7, 8, 999] {
            let status = segment.start_seek(target).unwrap();
            if matches!(status, DecoderSeekStatus::Pending) {
                assert_eq!(
                    segment.continue_seek().unwrap(),
                    DecoderSeekStatus::Complete(SeekResult {
                        actual_frame: target
                    })
                );
            }
            assert_eq!(
                collect(&mut segment),
                ((7 + target.min(8))..15)
                    .map(|v| v as f32)
                    .collect::<Vec<_>>()
            );
        }
    }
    #[test]
    fn adjacent_ranges_reconstruct_the_audible_source() {
        let mut samples = collect(&mut opened(0, 7));
        samples.extend(collect(&mut opened(7, 20)));
        assert_eq!(samples, (2..22).map(|v| v as f32).collect::<Vec<_>>());
    }
    #[test]
    fn rejects_out_of_bounds_or_changed_sample_rate() {
        for segment in [
            AudioSegment {
                start_frame: 0,
                end_frame_exclusive: 21,
                sample_rate: 48000,
            },
            AudioSegment {
                start_frame: 1,
                end_frame_exclusive: 1,
                sample_rate: 48000,
            },
            AudioSegment {
                start_frame: 0,
                end_frame_exclusive: 10,
                sample_rate: 44100,
            },
        ] {
            let mut decoder = SegmentDecoder::new(
                Box::new(Samples {
                    cursor: 0,
                    target: 0,
                }),
                segment,
            );
            assert!(
                decoder
                    .open(Box::new(EmptySource), &MediaHints::default())
                    .is_err()
            );
        }
    }
}
