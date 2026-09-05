use super::support::{TestDecoderFactory, TestSinkFactory, item};
use crate::planner::StageRegistrySnapshot;
use crate::playback::{
    control::SwitchOptions,
    runtime::{PlaybackRuntime, PlaybackRuntimeConfig},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use stellatune_audio_core::{
    error::{FactoryError, SinkError},
    format::{AudioBlock, PcmFormat},
    sink::{
        OutputCompatibilityKey, SinkClockSnapshot, SinkFactory, SinkStage, SinkWriteResult,
        SinkWriteState,
    },
    stage::StageId,
};
use tokio::time::{Duration, timeout};

#[derive(Default)]
struct Device {
    accepted: AtomicU64,
    consumed: AtomicU64,
    configured_ms: AtomicU64,
}
struct Factory {
    inner: TestSinkFactory,
    device: Arc<Device>,
}
impl SinkFactory for Factory {
    fn id(&self) -> &StageId {
        self.inner.id()
    }
    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        self.inner.compatibility_key(format)
    }
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(Sink(self.device.clone())))
    }
}
struct Sink(Arc<Device>);
impl SinkStage for Sink {
    fn configure_buffering(&mut self, config: stellatune_audio_core::buffering::BufferingConfig) {
        self.0
            .configured_ms
            .store(u64::from(config.output_ms), Ordering::Release);
    }
    fn open(&mut self, _: PcmFormat) -> Result<(), SinkError> {
        self.0.accepted.store(0, Ordering::Release);
        self.0.consumed.store(0, Ordering::Release);
        Ok(())
    }
    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        self.0
            .accepted
            .fetch_add(block.frames() as u64, Ordering::Release);
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
        self.0.accepted.store(0, Ordering::Release);
        self.0.consumed.store(0, Ordering::Release);
        Ok(())
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        let consumed = self.0.consumed.load(Ordering::Acquire);
        SinkClockSnapshot {
            consumed_frames: consumed,
            buffered_frames: self
                .0
                .accepted
                .load(Ordering::Acquire)
                .saturating_sub(consumed),
            epoch: 0,
        }
    }
    fn close(&mut self) {}
}
async fn accepted(device: &Device, frames: u64) {
    timeout(Duration::from_secs(1), async {
        while device.accepted.load(Ordering::Acquire) < frames {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn every_profile_reaches_its_duration_with_small_blocks_at_each_sample_rate() {
    use super::support::FixedFormatDecoderFactory;
    use stellatune_audio_core::{
        buffering::{LatencyProfile, frames_for_ms},
        format::ChannelLayout,
    };
    for rate in [8000, 44100, 48000, 96000, 192000] {
        for profile in [
            LatencyProfile::Low,
            LatencyProfile::Medium,
            LatencyProfile::High,
        ] {
            let format = PcmFormat {
                sample_rate: rate,
                channel_layout: ChannelLayout::STEREO,
            };
            let device = Arc::new(Device::default());
            let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
                decoders: vec![Arc::new(FixedFormatDecoderFactory::new(
                    "test.small-block",
                    format,
                    u64::from(rate) * 2,
                    0.1,
                ))],
                transforms: vec![],
                sink: Arc::new(Factory {
                    inner: TestSinkFactory {
                        id: StageId::new("test.duration").unwrap(),
                        samples: Arc::new(Mutex::new(vec![])),
                    },
                    device: device.clone(),
                }),
            });
            config.buffering = profile.buffering();
            let target = frames_for_ms(format, config.buffering.output_ms) as u64;
            let runtime = PlaybackRuntime::start(config).unwrap();
            let controller = runtime.controller();
            controller
                .switch_to(item(1, 200, 50), SwitchOptions::default())
                .await
                .unwrap();
            accepted(&device, target).await;
            tokio::time::sleep(Duration::from_millis(120)).await;
            let supplied = device.accepted.load(Ordering::Acquire);
            assert!(
                (target..target + 64).contains(&supplied),
                "{rate} Hz {profile:?}: target {target}, supplied {supplied}"
            );
            controller.pause().await.unwrap();
            device.consumed.store(supplied, Ordering::Release);
            tokio::time::sleep(Duration::from_millis(30)).await;
            assert_eq!(device.accepted.load(Ordering::Acquire), supplied);
            controller.play().await.unwrap();
            accepted(&device, supplied + target).await;
            runtime.shutdown().await.unwrap();
        }
    }
}

#[tokio::test]
async fn latency_change_is_deferred_until_output_rebuild_or_new_session() {
    use stellatune_audio_core::buffering::LatencyProfile;
    let device = Arc::new(Device::default());
    let config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(Factory {
            inner: TestSinkFactory {
                id: StageId::new("test.preset").unwrap(),
                samples: Arc::new(Mutex::new(vec![])),
            },
            device: device.clone(),
        }),
    });
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 200, 50), SwitchOptions::default())
        .await
        .unwrap();
    accepted(&device, 10).await;
    controller
        .set_latency_profile(LatencyProfile::High)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert_eq!(device.configured_ms.load(Ordering::Acquire), 100);
    assert_eq!(device.accepted.load(Ordering::Acquire), 10);
    controller.rebuild_output().await.unwrap();
    accepted(&device, 20).await;
    assert_eq!(device.configured_ms.load(Ordering::Acquire), 200);
    controller
        .set_latency_profile(LatencyProfile::Low)
        .await
        .unwrap();
    controller.stop().await.unwrap();
    controller
        .switch_to(item(2, 200, 50), SwitchOptions::default())
        .await
        .unwrap();
    accepted(&device, 4).await;
    assert_eq!(device.configured_ms.load(Ordering::Acquire), 40);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn pump_fills_media_time_target_then_waits_for_consumption_and_respects_pause() {
    let device = Arc::new(Device::default());
    let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(Factory {
            inner: TestSinkFactory {
                id: StageId::new("test.demand").unwrap(),
                samples: Arc::new(Mutex::new(vec![])),
            },
            device: device.clone(),
        }),
    });
    config.buffering.output_ms = 200; // 20 frames at the fixture's 100 Hz.
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 200, 100), SwitchOptions::default())
        .await
        .unwrap();
    accepted(&device, 20).await;
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        device.accepted.load(Ordering::Acquire),
        20,
        "a full device buffer must stop pumping, including on maintenance ticks"
    );
    device.consumed.store(5, Ordering::Release);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        device.accepted.load(Ordering::Acquire),
        20,
        "remaining audio above the low watermark must not trigger a refill"
    );
    device.consumed.store(15, Ordering::Release);
    accepted(&device, 40).await;
    assert_eq!(
        device.accepted.load(Ordering::Acquire),
        40,
        "refill stops at the target with at most one block of overshoot"
    );
    controller.pause().await.unwrap();
    device.consumed.store(35, Ordering::Release);
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(
        device.accepted.load(Ordering::Acquire),
        40,
        "readiness must not resume a paused track"
    );
    controller.play().await.unwrap();
    accepted(&device, 60).await;
    controller.stop().await.unwrap();
    runtime.shutdown().await.unwrap();
}
