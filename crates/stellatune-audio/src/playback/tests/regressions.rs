use super::support::{
    FixedFormatDecoderFactory, FormatAdaptingSinkFactory, TestDecoderFactory, TestSinkFactory,
    item, wait_for_end,
};
use crate::{
    planner::{PipelinePlanner, PlaybackRequest, StageRegistrySnapshot, TransitionPolicy},
    playback::{
        control::SwitchOptions,
        event::{PlaybackEvent, PlaybackState},
        preparation::prepare_off_turn,
        runtime::{PlaybackRuntime, PlaybackRuntimeConfig},
        state::PreparationPurpose,
        transition::normalize_prepared_for_mix,
    },
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use stellatune_audio_core::{
    decoder::{
        DecodeStatus, DecodedStreamInfo, DecoderDescriptor, DecoderFactory, DecoderSeekStatus,
        DecoderStage,
    },
    error::{DecodeError, FactoryError, FailureCode, FailureStage, PlaybackControlError},
    format::{AudioBlock, ChannelLayout, PcmFormat},
    playback::MediaTime,
    source::{EncodedSource, MediaHints, SourceCancellation, SourceOpenPurpose},
    stage::StageId,
};
use tokio::{sync::Semaphore, time::timeout};

fn config(samples: Arc<Mutex<Vec<f32>>>) -> PlaybackRuntimeConfig {
    let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(TestSinkFactory {
            id: StageId::new("test.sink").unwrap(),
            samples,
        }),
    });
    config.policies.transition = TransitionPolicy::Gapless;
    config.policies.seek_fade_frames = 0;
    config.max_pcm_blocks = 2;
    config
}
fn paused() -> SwitchOptions {
    SwitchOptions {
        autoplay: false,
        ..SwitchOptions::default()
    }
}

#[tokio::test]
async fn final_position_includes_the_short_tail_before_idle_and_end() {
    for seek_ms in [0, 500] {
        let samples = Arc::new(Mutex::new(vec![]));
        let runtime = PlaybackRuntime::start(config(Arc::clone(&samples))).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        let track = item(1, 103, 100);
        let item_id = track.id;
        controller.switch_to(track, paused()).await.unwrap();
        if seek_ms > 0 {
            controller
                .seek(MediaTime::from_millis(seek_ms))
                .await
                .unwrap();
        }
        controller.play().await.unwrap();
        let (last_position, position_at_idle) = timeout(Duration::from_secs(3), async {
            let mut last_position = None;
            let mut position_at_idle = None;
            loop {
                match events.recv().await.unwrap() {
                    PlaybackEvent::Position {
                        item_id: actual,
                        position,
                    } => {
                        assert_eq!(actual, item_id);
                        last_position = Some(position.as_millis());
                    },
                    PlaybackEvent::StateChanged(PlaybackState::Idle) => {
                        position_at_idle = last_position;
                    },
                    PlaybackEvent::PlaybackEnded { item_id: actual } => {
                        assert_eq!(actual, item_id);
                        break (last_position, position_at_idle);
                    },
                    PlaybackEvent::Failed(error) => panic!("{error}"),
                    _ => {},
                }
            }
        })
        .await
        .unwrap();
        runtime.shutdown().await.unwrap();
        assert_eq!(samples.lock().unwrap().len() as u64 * 10, 1030 - seek_ms);
        assert_eq!(last_position, Some(1030));
        assert_eq!(position_at_idle, Some(1030));
    }
}

#[tokio::test]
async fn seek_then_gapless_promotion_keeps_the_output_epoch_and_all_successor_pcm() {
    let samples = Arc::new(Mutex::new(vec![]));
    let runtime = PlaybackRuntime::start(config(Arc::clone(&samples))).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch_to(item(1, 100, 100), paused())
        .await
        .unwrap();
    controller.set_next(Some(item(2, 40, 50))).await.unwrap();
    controller.seek(MediaTime::from_millis(500)).await.unwrap();
    controller.play().await.unwrap();
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
    let samples = samples.lock().unwrap();
    assert_eq!(samples.len(), 90);
    assert_eq!(&samples[..50], vec![1.0; 50]);
    assert_eq!(&samples[50..], vec![0.5; 40]);
}

#[tokio::test]
async fn rebuilding_after_seek_retains_position_and_decodes_the_remaining_pcm_once() {
    let samples = Arc::new(Mutex::new(vec![]));
    let runtime = PlaybackRuntime::start(config(Arc::clone(&samples))).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch_to(item(1, 100, 100), paused())
        .await
        .unwrap();
    controller.seek(MediaTime::from_millis(500)).await.unwrap();
    controller.rebuild_output().await.unwrap();
    let snapshot = controller.snapshot().await.unwrap();
    assert_eq!(snapshot.consumed_position, MediaTime::from_millis(500));
    assert_eq!(snapshot.state, PlaybackState::Paused);
    controller.play().await.unwrap();
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
    assert_eq!(samples.lock().unwrap().as_slice(), vec![1.0; 50]);
}

#[tokio::test]
async fn seek_position_uses_decoded_rate_before_converting_to_mix_frames() {
    let mut config = config(Arc::new(Mutex::new(vec![])));
    config.registry.decoders = vec![Arc::new(FixedFormatDecoderFactory::new(
        "test.96k",
        PcmFormat {
            sample_rate: 96_000,
            channel_layout: ChannelLayout::MONO,
        },
        192_000,
        1.0,
    ))];
    config.registry.sink = Arc::new(FormatAdaptingSinkFactory {
        id: StageId::new("test.48k").unwrap(),
        target: PcmFormat {
            sample_rate: 48_000,
            channel_layout: ChannelLayout::MONO,
        },
        formats: Arc::new(Mutex::new(vec![])),
        samples: Arc::new(Mutex::new(vec![])),
    });
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 100), paused())
        .await
        .unwrap();
    controller.seek(MediaTime::from_millis(500)).await.unwrap();
    assert_eq!(
        controller.snapshot().await.unwrap().consumed_position,
        MediaTime::from_millis(500)
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn replacing_a_prepared_normalizer_uses_the_original_transform_output_rate() {
    let mut config = config(Arc::new(Mutex::new(vec![])));
    config.registry.decoders = vec![Arc::new(FixedFormatDecoderFactory::new(
        "test.96k",
        PcmFormat {
            sample_rate: 96_000,
            channel_layout: ChannelLayout::MONO,
        },
        9_600,
        1.0,
    ))];
    config.registry.sink = Arc::new(FormatAdaptingSinkFactory {
        id: StageId::new("test.48k").unwrap(),
        target: PcmFormat {
            sample_rate: 48_000,
            channel_layout: ChannelLayout::MONO,
        },
        formats: Arc::new(Mutex::new(vec![])),
        samples: Arc::new(Mutex::new(vec![])),
    });
    let plan = PipelinePlanner
        .plan(
            PlaybackRequest {
                item: item(1, 100, 100),
                policies: config.policies,
            },
            &config.registry,
        )
        .unwrap();
    let mut prepared = prepare_off_turn(
        plan,
        1,
        1,
        SourceOpenPurpose::Initial,
        PreparationPurpose::Current,
        SourceCancellation::default(),
        Instant::now() + Duration::from_secs(2),
    )
    .await
    .result
    .unwrap();
    normalize_prepared_for_mix(
        &mut prepared,
        PcmFormat {
            sample_rate: 44_100,
            channel_layout: ChannelLayout::MONO,
        },
    )
    .unwrap();
    let mut frames = 0;
    loop {
        match prepared.pipeline.decode(Default::default(), 0).unwrap() {
            crate::playback::pipeline::TrackBlockStatus::Data(block) => frames += block.frames(),
            crate::playback::pipeline::TrackBlockStatus::Pending
            | crate::playback::pipeline::TrackBlockStatus::Progress => {},
            crate::playback::pipeline::TrackBlockStatus::EndOfStream => break,
        }
    }
    let normalizer = prepared.pipeline.normalizer.as_mut().unwrap();
    let mut tail = AudioBlock::new(prepared.pipeline.mix_format);
    while normalizer.drain(&mut tail).unwrap() {
        frames += tail.frames();
    }
    assert_eq!(frames, 4_410);
}

struct PendingDecoderFactory {
    inner: TestDecoderFactory,
    gate: Arc<AtomicBool>,
    started: Arc<Semaphore>,
    continuations: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
}
impl DecoderFactory for PendingDecoderFactory {
    fn descriptor(&self) -> &DecoderDescriptor {
        self.inner.descriptor()
    }
    fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
        Ok(Box::new(PendingDecoder {
            inner: self.inner.create()?,
            gate: Arc::clone(&self.gate),
            started: Arc::clone(&self.started),
            continuations: Arc::clone(&self.continuations),
            dropped: Arc::clone(&self.dropped),
            target: 0,
            pending_decode: true,
        }))
    }
}
struct PendingDecoder {
    inner: Box<dyn DecoderStage>,
    gate: Arc<AtomicBool>,
    started: Arc<Semaphore>,
    continuations: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
    target: u64,
    pending_decode: bool,
}
impl Drop for PendingDecoder {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}
impl DecoderStage for PendingDecoder {
    fn open(
        &mut self,
        source: Box<dyn EncodedSource>,
        hints: &MediaHints,
    ) -> Result<DecodedStreamInfo, DecodeError> {
        self.inner.open(source, hints)
    }
    fn decode(&mut self, block: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
        if std::mem::take(&mut self.pending_decode) {
            return Ok(DecodeStatus::Pending);
        }
        self.inner.decode(block)
    }
    fn start_seek(&mut self, target: u64) -> Result<DecoderSeekStatus, DecodeError> {
        self.target = target;
        self.started.add_permits(1);
        Ok(DecoderSeekStatus::Pending)
    }
    fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
        self.continuations.fetch_add(1, Ordering::Relaxed);
        if self.gate.load(Ordering::Acquire) {
            self.inner.start_seek(self.target)
        } else {
            Ok(DecoderSeekStatus::Pending)
        }
    }
    fn reset(&mut self) {
        self.inner.reset();
    }
}
fn pending_factory() -> PendingDecoderFactory {
    PendingDecoderFactory {
        inner: TestDecoderFactory::new(),
        gate: Arc::new(AtomicBool::new(false)),
        started: Arc::new(Semaphore::new(0)),
        continuations: Arc::new(AtomicUsize::new(0)),
        dropped: Arc::new(AtomicBool::new(false)),
    }
}
#[tokio::test]
async fn pause_during_pending_seek_wins_and_decode_pending_remains_nonfatal() {
    let samples = Arc::new(Mutex::new(vec![]));
    let mut config = config(Arc::clone(&samples));
    let factory = pending_factory();
    let gate = Arc::clone(&factory.gate);
    let started = Arc::clone(&factory.started);
    config.registry.decoders = vec![Arc::new(factory)];
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch_to(item(1, 100, 100), paused())
        .await
        .unwrap();
    controller.play().await.unwrap();
    let seek = tokio::spawn({
        let controller = controller.clone();
        async move { controller.seek(MediaTime::from_millis(500)).await }
    });
    timeout(Duration::from_secs(2), started.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    controller.pause().await.unwrap();
    gate.store(true, Ordering::Release);
    seek.await.unwrap().unwrap();
    assert_eq!(
        controller.snapshot().await.unwrap().state,
        PlaybackState::Paused
    );
    samples.lock().unwrap().clear();
    controller.play().await.unwrap();
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
    assert_eq!(samples.lock().unwrap().len(), 50);
}

#[tokio::test]
async fn recovery_seek_obeys_cancellation_and_the_preparation_deadline() {
    for cancel_explicitly in [true, false] {
        let mut config = config(Arc::new(Mutex::new(vec![])));
        let factory = pending_factory();
        let started = Arc::clone(&factory.started);
        let dropped = Arc::clone(&factory.dropped);
        config.registry.decoders = vec![Arc::new(factory)];
        let plan = PipelinePlanner
            .plan(
                PlaybackRequest {
                    item: item(1, 100, 100),
                    policies: config.policies,
                },
                &config.registry,
            )
            .unwrap();
        let item_id = plan.item.id;
        let cancellation = SourceCancellation::default();
        let task = tokio::spawn(prepare_off_turn(
            plan,
            1,
            1,
            SourceOpenPurpose::Recovery,
            PreparationPurpose::Recovery {
                item_id,
                checkpoint: MediaTime::from_millis(500),
                attempt: 1,
            },
            cancellation.clone(),
            Instant::now() + Duration::from_millis(100),
        ));
        timeout(Duration::from_secs(2), started.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
        if cancel_explicitly {
            cancellation.cancel();
        }
        let result = timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap()
            .result;
        if cancel_explicitly {
            assert!(matches!(result, Err(PlaybackControlError::Closed)));
        } else {
            assert!(matches!(
                result,
                Err(PlaybackControlError::CommandTimeout { .. })
            ));
        }
        timeout(Duration::from_secs(1), async {
            while !dropped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}

#[test]
fn stage_errors_keep_category_and_implementation_identity() {
    let id = StageId::new("test.decoder").unwrap();
    let error = PlaybackControlError::decoder(DecodeError::Unsupported, id.clone());
    let PlaybackControlError::Failed(failure) = error else {
        panic!("expected typed failure")
    };
    assert_eq!(failure.stage, FailureStage::Decoder);
    assert_eq!(failure.code, FailureCode::Unsupported);
    assert_eq!(failure.stage_id, Some(id));
}

struct RecoverySource {
    inner: Arc<dyn stellatune_audio_core::source::SourceFactory>,
    opened: Arc<Semaphore>,
    release: Arc<Semaphore>,
}
impl stellatune_audio_core::source::SourceFactory for RecoverySource {
    fn descriptor(&self) -> stellatune_audio_core::source::SourceDescriptor {
        self.inner.descriptor()
    }
    fn open(
        &self,
        request: stellatune_audio_core::source::SourceOpenRequest,
    ) -> stellatune_audio_core::source::SourceOpenFuture<'_> {
        Box::pin(async move {
            if request.purpose == SourceOpenPurpose::Recovery {
                self.opened.add_permits(1);
                self.release.acquire().await.unwrap().forget();
            }
            self.inner.open(request).await
        })
    }
}
#[tokio::test]
async fn pause_during_recovery_overrides_the_playing_state_that_started_recovery() {
    let samples = Arc::new(Mutex::new(vec![]));
    let mut config = config(Arc::clone(&samples));
    let creates = Arc::new(AtomicUsize::new(0));
    config.registry.sink = Arc::new(super::support::RecoveringSinkFactory {
        id: StageId::new("test.recovery").unwrap(),
        samples,
        creates: Arc::clone(&creates),
    });
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    let opened = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let mut track = item(1, 100, 100);
    track.source = Arc::new(RecoverySource {
        inner: track.source,
        opened: Arc::clone(&opened),
        release: Arc::clone(&release),
    });
    controller
        .switch_to(track, SwitchOptions::default())
        .await
        .unwrap();
    timeout(Duration::from_secs(2), opened.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    controller.pause().await.unwrap();
    release.add_permits(1);
    timeout(Duration::from_secs(2), async {
        while creates.load(Ordering::Acquire) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        controller.snapshot().await.unwrap().state,
        PlaybackState::Paused
    );
    runtime.shutdown().await.unwrap();
}

struct BackgroundSource {
    inner: Arc<dyn stellatune_audio_core::source::SourceFactory>,
    release: Arc<Semaphore>,
    alive: Arc<Semaphore>,
}
impl stellatune_audio_core::source::SourceFactory for BackgroundSource {
    fn descriptor(&self) -> stellatune_audio_core::source::SourceDescriptor {
        self.inner.descriptor()
    }
    fn open(
        &self,
        request: stellatune_audio_core::source::SourceOpenRequest,
    ) -> stellatune_audio_core::source::SourceOpenFuture<'_> {
        Box::pin(async move {
            let release = Arc::clone(&self.release);
            let alive = Arc::clone(&self.alive);
            tokio::spawn(async move {
                release.acquire().await.unwrap().forget();
                alive.add_permits(1);
            });
            self.inner.open(request).await
        })
    }
}
#[tokio::test]
async fn source_background_io_survives_preparation_completion() {
    let config = config(Arc::new(Mutex::new(vec![])));
    let release = Arc::new(Semaphore::new(0));
    let alive = Arc::new(Semaphore::new(0));
    let mut track = item(1, 100, 100);
    track.source = Arc::new(BackgroundSource {
        inner: track.source,
        release: Arc::clone(&release),
        alive: Arc::clone(&alive),
    });
    let plan = PipelinePlanner
        .plan(
            PlaybackRequest {
                item: track,
                policies: config.policies,
            },
            &config.registry,
        )
        .unwrap();
    let prepared = prepare_off_turn(
        plan,
        1,
        1,
        SourceOpenPurpose::Initial,
        PreparationPurpose::Current,
        SourceCancellation::default(),
        Instant::now() + Duration::from_secs(2),
    )
    .await
    .result
    .unwrap();
    release.add_permits(1);
    timeout(Duration::from_secs(1), alive.acquire())
        .await
        .expect("source executor must outlive preparation")
        .unwrap()
        .forget();
    drop(prepared);
}
