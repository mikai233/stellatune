use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use stellatune_audio_core::{
    decoder::{
        DecodeStatus, DecodedStreamInfo, DecoderDescriptor, DecoderFactory, DecoderSeekStatus,
        DecoderStage, SeekResult,
    },
    error::{DecodeError, FactoryError, SinkError, SourceError, TransformError},
    format::{AudioBlock, ChannelLayout, PcmFormat},
    playback::{PlaybackItem, PlaybackItemId},
    sink::{
        OutputCompatibilityKey, SinkClockSnapshot, SinkFactory, SinkStage, SinkWriteResult,
        SinkWriteState,
    },
    source::{
        EncodedSource, MediaHints, MemorySourceFactory, SourceCapabilities, SourceDescriptor,
        SourceFactory, SourceOpenFuture, SourceOpenRequest,
    },
    stage::StageId,
    transform::{
        DrainStatus, TransformDescriptor, TransformFactory, TransformPlacement, TransformStage,
        TransformStatus,
    },
};
use tokio::sync::{Semaphore, broadcast};
use tokio::time::{Duration, timeout};

use crate::planner::{StageRegistrySnapshot, TransitionPolicy};
use crate::playback::event::PlaybackEvent;
use crate::playback::runtime::{PlaybackRuntime, PlaybackRuntimeConfig};

pub(in crate::playback) struct TestDecoderFactory {
    pub(super) descriptor: DecoderDescriptor,
}

pub(super) struct CountingTransformFactory {
    pub(super) descriptor: TransformDescriptor,
    pub(super) process_counts: Arc<Mutex<Vec<usize>>>,
}

impl CountingTransformFactory {
    pub(super) fn new(
        id: &str,
        placement: TransformPlacement,
        process_counts: Arc<Mutex<Vec<usize>>>,
    ) -> Self {
        Self {
            descriptor: TransformDescriptor {
                id: StageId::new(id).unwrap(),
                placement,
            },
            process_counts,
        }
    }
}

impl TransformFactory for CountingTransformFactory {
    fn descriptor(&self) -> &TransformDescriptor {
        &self.descriptor
    }

    fn create(&self) -> Result<Box<dyn TransformStage>, FactoryError> {
        let index = {
            let mut counts = self.process_counts.lock().unwrap();
            counts.push(0);
            counts.len() - 1
        };
        Ok(Box::new(CountingTransform {
            index,
            process_counts: Arc::clone(&self.process_counts),
        }))
    }
}

pub(super) struct CountingTransform {
    pub(super) index: usize,
    pub(super) process_counts: Arc<Mutex<Vec<usize>>>,
}

pub(super) struct BufferingTailTransformFactory {
    pub(super) descriptor: TransformDescriptor,
}

impl BufferingTailTransformFactory {
    pub(super) fn new(placement: TransformPlacement) -> Self {
        Self {
            descriptor: TransformDescriptor {
                id: StageId::new(match placement {
                    TransformPlacement::PreMix => "test.buffering-pre",
                    TransformPlacement::PostMix => "test.buffering-post",
                })
                .unwrap(),
                placement,
            },
        }
    }
}

impl TransformFactory for BufferingTailTransformFactory {
    fn descriptor(&self) -> &TransformDescriptor {
        &self.descriptor
    }

    fn create(&self) -> Result<Box<dyn TransformStage>, FactoryError> {
        Ok(Box::new(BufferingTailTransform { buffered: None }))
    }
}

pub(super) struct BufferingTailTransform {
    pub(super) buffered: Option<Vec<f32>>,
}

impl TransformStage for BufferingTailTransform {
    fn configure(&mut self, input: PcmFormat) -> Result<PcmFormat, TransformError> {
        Ok(input)
    }

    fn process(&mut self, block: &mut AudioBlock) -> Result<TransformStatus, TransformError> {
        self.buffered = Some(block.samples.clone());
        Ok(TransformStatus::Buffered)
    }

    fn drain(&mut self, output: &mut AudioBlock) -> Result<DrainStatus, TransformError> {
        match self.buffered.take() {
            Some(samples) => {
                output.samples = samples;
                Ok(DrainStatus::Produced)
            },
            None => Ok(DrainStatus::Complete),
        }
    }

    fn reset(&mut self) {
        self.buffered = None;
    }
}

impl TransformStage for CountingTransform {
    fn configure(&mut self, input: PcmFormat) -> Result<PcmFormat, TransformError> {
        Ok(input)
    }

    fn process(&mut self, _block: &mut AudioBlock) -> Result<TransformStatus, TransformError> {
        self.process_counts.lock().unwrap()[self.index] += 1;
        Ok(TransformStatus::Produced)
    }

    fn drain(&mut self, _output: &mut AudioBlock) -> Result<DrainStatus, TransformError> {
        Ok(DrainStatus::Complete)
    }

    fn reset(&mut self) {}
}

impl TestDecoderFactory {
    pub(in crate::playback) fn new() -> Self {
        Self {
            descriptor: DecoderDescriptor {
                id: StageId::new("test.decoder").unwrap(),
                priority: 1,
                extensions: Vec::new(),
                mime_types: Vec::new(),
            },
        }
    }
}

impl DecoderFactory for TestDecoderFactory {
    fn descriptor(&self) -> &DecoderDescriptor {
        &self.descriptor
    }

    fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
        Ok(Box::new(TestDecoder {
            remaining: 0,
            total: 0,
            amplitude: 0.0,
        }))
    }
}

pub(super) struct TestDecoder {
    pub(super) remaining: u64,
    pub(super) total: u64,
    pub(super) amplitude: f32,
}

pub(super) struct FixedFormatDecoderFactory {
    pub(super) descriptor: DecoderDescriptor,
    pub(super) format: PcmFormat,
    pub(super) frames: u64,
    pub(super) amplitude: f32,
}

impl FixedFormatDecoderFactory {
    pub(super) fn new(id: &str, format: PcmFormat, frames: u64, amplitude: f32) -> Self {
        Self {
            descriptor: DecoderDescriptor {
                id: StageId::new(id).unwrap(),
                priority: 10,
                extensions: Vec::new(),
                mime_types: Vec::new(),
            },
            format,
            frames,
            amplitude,
        }
    }
}

impl DecoderFactory for FixedFormatDecoderFactory {
    fn descriptor(&self) -> &DecoderDescriptor {
        &self.descriptor
    }

    fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
        Ok(Box::new(FixedFormatDecoder {
            format: self.format,
            total: self.frames,
            remaining: self.frames,
            amplitude: self.amplitude,
        }))
    }
}

pub(super) struct FixedFormatDecoder {
    pub(super) format: PcmFormat,
    pub(super) total: u64,
    pub(super) remaining: u64,
    pub(super) amplitude: f32,
}

pub(super) struct FailingNextDecoderFactory {
    pub(super) descriptor: DecoderDescriptor,
}

impl FailingNextDecoderFactory {
    pub(super) fn new() -> Self {
        Self {
            descriptor: DecoderDescriptor {
                id: StageId::new("test.failing-next").unwrap(),
                priority: 10,
                extensions: Vec::new(),
                mime_types: Vec::new(),
            },
        }
    }
}

impl DecoderFactory for FailingNextDecoderFactory {
    fn descriptor(&self) -> &DecoderDescriptor {
        &self.descriptor
    }

    fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
        Ok(Box::new(FailingNextDecoder { emitted: false }))
    }
}

pub(super) struct FailingNextDecoder {
    pub(super) emitted: bool,
}

impl DecoderStage for FailingNextDecoder {
    fn open(
        &mut self,
        _source: Box<dyn EncodedSource>,
        _hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        Ok(DecodedStreamInfo {
            format: PcmFormat {
                sample_rate: 100,
                channel_layout: ChannelLayout::MONO,
            },
            duration_frames: Some(100),
            gapless_trim: None,
        })
    }

    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        if self.emitted {
            return Err(DecodeError::Failed {
                message: "simulated next decoder failure".to_owned(),
            });
        }
        self.emitted = true;
        output.samples = vec![0.0; 10];
        Ok(DecodeStatus::Produced { frames: 10 })
    }

    fn start_seek(&mut self, _target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
        Err(DecodeError::Unsupported)
    }

    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        Err(DecodeError::Unsupported)
    }

    fn reset(&mut self) {}
}

impl DecoderStage for FixedFormatDecoder {
    fn open(
        &mut self,
        _source: Box<dyn EncodedSource>,
        _hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        Ok(DecodedStreamInfo {
            format: self.format,
            duration_frames: Some(self.total),
            gapless_trim: None,
        })
    }

    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        if self.remaining == 0 {
            return Ok(DecodeStatus::EndOfStream);
        }
        let frames = self.remaining.min(64) as usize;
        output.samples.resize(
            frames.saturating_mul(usize::from(self.format.channel_layout.channel_count())),
            self.amplitude,
        );
        self.remaining -= frames as u64;
        Ok(DecodeStatus::Produced { frames })
    }

    fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
        let actual = target_frame.min(self.total);
        self.remaining = self.total.saturating_sub(actual);
        Ok(DecoderSeekStatus::Complete(SeekResult {
            actual_frame: actual,
        }))
    }

    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        Err(DecodeError::Unsupported)
    }

    fn reset(&mut self) {}
}

impl DecoderStage for TestDecoder {
    fn open(
        &mut self,
        mut source: Box<dyn EncodedSource>,
        _hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        let mut input = [0_u8; 2];
        source.read_exact(&mut input).map_err(DecodeError::Io)?;
        self.total = u64::from(input[0]);
        self.remaining = self.total;
        self.amplitude = f32::from(input[1]) / 100.0;
        Ok(DecodedStreamInfo {
            format: PcmFormat {
                sample_rate: 100,
                channel_layout: ChannelLayout::MONO,
            },
            duration_frames: Some(self.total),
            gapless_trim: None,
        })
    }

    fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        if self.remaining == 0 {
            return Ok(DecodeStatus::EndOfStream);
        }
        let frames = self.remaining.min(10) as usize;
        output.samples.resize(frames, self.amplitude);
        self.remaining -= frames as u64;
        Ok(DecodeStatus::Produced { frames })
    }

    fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
        self.remaining = self.total.saturating_sub(target_frame);
        Ok(DecoderSeekStatus::Complete(SeekResult {
            actual_frame: target_frame.min(self.total),
        }))
    }

    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        Err(DecodeError::Unsupported)
    }

    fn reset(&mut self) {}
}

pub(in crate::playback) struct TestSinkFactory {
    pub(in crate::playback) id: StageId,
    pub(in crate::playback) samples: Arc<Mutex<Vec<f32>>>,
}

impl SinkFactory for TestSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }

    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        Ok(OutputCompatibilityKey {
            backend_id: "test".to_owned(),
            device_id: None,
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: 0,
        })
    }

    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(TestSink {
            samples: Arc::clone(&self.samples),
            consumed: 0,
            epoch: 0,
        }))
    }
}

pub(super) struct TestSink {
    pub(super) samples: Arc<Mutex<Vec<f32>>>,
    pub(super) consumed: u64,
    pub(super) epoch: u64,
}

pub(super) struct RecordingSinkFactory {
    pub(super) id: StageId,
    pub(super) formats: Arc<Mutex<Vec<PcmFormat>>>,
    pub(super) creates: Arc<AtomicUsize>,
}

pub(super) struct FormatAdaptingSinkFactory {
    pub(super) id: StageId,
    pub(super) target: PcmFormat,
    pub(super) formats: Arc<Mutex<Vec<PcmFormat>>>,
    pub(super) samples: Arc<Mutex<Vec<f32>>>,
}

impl SinkFactory for FormatAdaptingSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }

    fn preferred_format(&self, _input: PcmFormat) -> Result<PcmFormat, FactoryError> {
        Ok(self.target)
    }

    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        Ok(OutputCompatibilityKey {
            backend_id: "format-adapting".to_owned(),
            device_id: None,
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: 0,
        })
    }

    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(FormatAdaptingSink {
            formats: Arc::clone(&self.formats),
            samples: Arc::clone(&self.samples),
            consumed: 0,
        }))
    }
}

pub(super) struct FormatAdaptingSink {
    pub(super) formats: Arc<Mutex<Vec<PcmFormat>>>,
    pub(super) samples: Arc<Mutex<Vec<f32>>>,
    pub(super) consumed: u64,
}

impl SinkStage for FormatAdaptingSink {
    fn open(&mut self, format: PcmFormat) -> Result<(), SinkError> {
        self.formats.lock().unwrap().push(format);
        Ok(())
    }

    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        assert_eq!(block.format, *self.formats.lock().unwrap().last().unwrap());
        self.samples
            .lock()
            .unwrap()
            .extend_from_slice(&block.samples);
        self.consumed = self.consumed.saturating_add(block.frames() as u64);
        Ok(SinkWriteResult {
            consumed_frames: block.frames(),
            state: SinkWriteState::Ready,
        })
    }

    fn pause(&mut self) -> Result<(), SinkError> {
        Ok(())
    }

    fn resume(&mut self) -> Result<(), SinkError> {
        Ok(())
    }

    fn drain(&mut self) -> Result<(), SinkError> {
        Ok(())
    }

    fn discard(&mut self) -> Result<(), SinkError> {
        self.consumed = 0;
        Ok(())
    }

    fn clock_snapshot(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            consumed_frames: self.consumed,
            buffered_frames: 0,
            epoch: 0,
        }
    }

    fn close(&mut self) {}
}

impl SinkFactory for RecordingSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }

    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        Ok(OutputCompatibilityKey {
            backend_id: "recording".to_owned(),
            device_id: None,
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: 0,
        })
    }

    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(RecordingSink {
            formats: Arc::clone(&self.formats),
            consumed: 0,
        }))
    }
}

pub(super) struct RecordingSink {
    pub(super) formats: Arc<Mutex<Vec<PcmFormat>>>,
    pub(super) consumed: u64,
}

pub(super) struct StalledSinkFactory {
    pub(super) id: StageId,
    pub(super) allow_writes: Arc<std::sync::atomic::AtomicBool>,
}

impl SinkFactory for StalledSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }

    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        Ok(OutputCompatibilityKey {
            backend_id: "stalled".to_owned(),
            device_id: None,
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: 0,
        })
    }

    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(StalledSink {
            allow_writes: Arc::clone(&self.allow_writes),
            consumed: 0,
        }))
    }
}

pub(super) struct StalledSink {
    pub(super) allow_writes: Arc<std::sync::atomic::AtomicBool>,
    pub(super) consumed: u64,
}

impl SinkStage for StalledSink {
    fn open(&mut self, _format: PcmFormat) -> Result<(), SinkError> {
        Ok(())
    }
    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        if !self.allow_writes.load(Ordering::SeqCst) {
            return Ok(SinkWriteResult {
                consumed_frames: 0,
                state: SinkWriteState::WouldBlock,
            });
        }
        self.consumed = self.consumed.saturating_add(block.frames() as u64);
        Ok(SinkWriteResult {
            consumed_frames: block.frames(),
            state: SinkWriteState::Ready,
        })
    }
    fn pause(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn drain(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn discard(&mut self) -> Result<(), SinkError> {
        self.consumed = 0;
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            consumed_frames: self.consumed,
            buffered_frames: 0,
            epoch: 0,
        }
    }
    fn close(&mut self) {}
}

impl SinkStage for RecordingSink {
    fn open(&mut self, _format: PcmFormat) -> Result<(), SinkError> {
        Ok(())
    }
    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        self.formats.lock().unwrap().push(block.format);
        self.consumed = self.consumed.saturating_add(block.frames() as u64);
        Ok(SinkWriteResult {
            consumed_frames: block.frames(),
            state: SinkWriteState::Ready,
        })
    }
    fn pause(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn drain(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn discard(&mut self) -> Result<(), SinkError> {
        self.consumed = 0;
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            consumed_frames: self.consumed,
            buffered_frames: 0,
            epoch: 0,
        }
    }
    fn close(&mut self) {}
}

pub(super) struct RecoveringSinkFactory {
    pub(super) id: StageId,
    pub(super) samples: Arc<Mutex<Vec<f32>>>,
    pub(super) creates: Arc<AtomicUsize>,
}

#[derive(Clone)]
pub(super) struct DelayedSourceFactory {
    pub(super) inner: MemorySourceFactory,
    pub(super) delay: Duration,
    pub(super) entered: Option<Arc<Semaphore>>,
}

impl SourceFactory for DelayedSourceFactory {
    fn descriptor(&self) -> SourceDescriptor {
        self.inner.descriptor()
    }

    fn open(&self, request: SourceOpenRequest) -> SourceOpenFuture<'_> {
        let inner = self.inner.clone();
        let delay = self.delay;
        let entered = self.entered.clone();
        let cancellation = request.cancellation.clone();
        Box::pin(async move {
            if let Some(entered) = entered {
                entered.add_permits(1);
            }
            tokio::select! {
                () = tokio::time::sleep(delay) => inner.open(request).await,
                () = cancellation.cancelled() => Err(SourceError::Cancelled),
            }
        })
    }
}

impl SinkFactory for RecoveringSinkFactory {
    fn id(&self) -> &StageId {
        &self.id
    }

    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        Ok(OutputCompatibilityKey {
            backend_id: "recovering-test".to_owned(),
            device_id: None,
            sample_rate: format.sample_rate,
            channel_layout: format.channel_layout,
            route_revision: 0,
        })
    }

    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        let instance = self.creates.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(RecoveringSink {
            samples: Arc::clone(&self.samples),
            consumed: 0,
            writes: 0,
            fail_on_second_write: instance == 0,
        }))
    }
}

pub(super) struct RecoveringSink {
    pub(super) samples: Arc<Mutex<Vec<f32>>>,
    pub(super) consumed: u64,
    pub(super) writes: usize,
    pub(super) fail_on_second_write: bool,
}

impl SinkStage for RecoveringSink {
    fn open(&mut self, _format: PcmFormat) -> Result<(), SinkError> {
        Ok(())
    }

    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        self.writes += 1;
        if self.fail_on_second_write && self.writes == 2 {
            return Err(SinkError::Failed {
                message: "simulated device disconnect".to_owned(),
            });
        }
        self.samples
            .lock()
            .unwrap()
            .extend_from_slice(&block.samples);
        self.consumed = self.consumed.saturating_add(block.frames() as u64);
        Ok(SinkWriteResult {
            consumed_frames: block.frames(),
            state: SinkWriteState::Ready,
        })
    }

    fn pause(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn drain(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn discard(&mut self) -> Result<(), SinkError> {
        self.consumed = 0;
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            consumed_frames: self.consumed,
            buffered_frames: 0,
            epoch: 0,
        }
    }
    fn close(&mut self) {}
}

impl SinkStage for TestSink {
    fn open(&mut self, _format: PcmFormat) -> Result<(), SinkError> {
        Ok(())
    }

    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        self.samples
            .lock()
            .unwrap()
            .extend_from_slice(&block.samples);
        self.consumed = self.consumed.saturating_add(block.frames() as u64);
        Ok(SinkWriteResult {
            consumed_frames: block.frames(),
            state: SinkWriteState::Ready,
        })
    }

    fn pause(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn resume(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn drain(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
    fn discard(&mut self) -> Result<(), SinkError> {
        self.consumed = 0;
        self.epoch = self.epoch.wrapping_add(1);
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            consumed_frames: self.consumed,
            buffered_frames: 0,
            epoch: self.epoch,
        }
    }
    fn close(&mut self) {}
}

pub(super) fn item(id: u64, frames: u8, amplitude: u8) -> PlaybackItem {
    PlaybackItem {
        id: PlaybackItemId::new(id).unwrap(),
        source: Arc::new(MemorySourceFactory::new(
            Arc::<[u8]>::from([frames, amplitude]),
            SourceDescriptor {
                media: MediaHints::default(),
                capabilities: SourceCapabilities {
                    byte_seekable: true,
                    reopenable: true,
                    live: false,
                },
            },
        )),
        required_decoder: None,
    }
}

pub(super) fn fixed_format_item(id: u64, factory: Arc<dyn DecoderFactory>) -> PlaybackItem {
    PlaybackItem {
        id: PlaybackItemId::new(id).unwrap(),
        source: Arc::new(MemorySourceFactory::new(
            Arc::<[u8]>::from([0_u8]),
            SourceDescriptor {
                media: MediaHints::default(),
                capabilities: SourceCapabilities {
                    byte_seekable: true,
                    reopenable: true,
                    live: false,
                },
            },
        )),
        required_decoder: Some(factory),
    }
}

pub(in crate::playback) fn delayed_item(
    id: u64,
    frames: u8,
    amplitude: u8,
    delay: Duration,
) -> PlaybackItem {
    PlaybackItem {
        id: PlaybackItemId::new(id).unwrap(),
        source: Arc::new(DelayedSourceFactory {
            inner: MemorySourceFactory::new(
                Arc::<[u8]>::from([frames, amplitude]),
                SourceDescriptor {
                    media: MediaHints::default(),
                    capabilities: SourceCapabilities {
                        byte_seekable: true,
                        reopenable: true,
                        live: false,
                    },
                },
            ),
            delay,
            entered: None,
        }),
        required_decoder: None,
    }
}

pub(in crate::playback) fn signaled_delayed_item(
    id: u64,
    frames: u8,
    amplitude: u8,
    delay: Duration,
    entered: Arc<Semaphore>,
) -> PlaybackItem {
    let mut item = delayed_item(id, frames, amplitude, delay);
    item.source = Arc::new(DelayedSourceFactory {
        inner: MemorySourceFactory::new(
            Arc::<[u8]>::from([frames, amplitude]),
            SourceDescriptor {
                media: MediaHints::default(),
                capabilities: SourceCapabilities {
                    byte_seekable: true,
                    reopenable: true,
                    live: false,
                },
            },
        ),
        delay,
        entered: Some(entered),
    });
    item
}

pub(in crate::playback) fn runtime(
    transition: TransitionPolicy,
    samples: Arc<Mutex<Vec<f32>>>,
) -> PlaybackRuntime {
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: Vec::new(),
        sink: Arc::new(TestSinkFactory {
            id: StageId::new("test.sink").unwrap(),
            samples,
        }),
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.max_pcm_blocks = 2;
    config.policies.transition = transition;
    PlaybackRuntime::start(config).unwrap()
}

pub(in crate::playback) async fn wait_for_end(events: &mut broadcast::Receiver<PlaybackEvent>) {
    timeout(Duration::from_secs(3), async {
        loop {
            if matches!(
                events.recv().await.unwrap(),
                PlaybackEvent::PlaybackEnded { .. }
            ) {
                break;
            }
        }
    })
    .await
    .expect("playback should end");
}
