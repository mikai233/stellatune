use super::support::{TestDecoderFactory, TestSinkFactory, item, wait_for_end};
use crate::{
    planner::StageRegistrySnapshot,
    playback::{
        control::SwitchOptions,
        event::{PlaybackEvent, PlaybackState},
        runtime::{PlaybackRuntime, PlaybackRuntimeConfig},
    },
};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use stellatune_audio_core::{
    error::{FactoryError, PlaybackControlError, SinkError},
    format::{AudioBlock, PcmFormat},
    sink::{OutputCompatibilityKey, SinkClockSnapshot, SinkFactory, SinkStage, SinkWriteResult},
    stage::StageId,
};
use tokio::{sync::Semaphore, time::timeout};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Operation {
    Open,
    Resume,
    Drain,
    DrainIncompatible,
    Close,
}
struct Gate {
    released: Mutex<bool>,
    wake: Condvar,
    entered: Semaphore,
}
impl Gate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            released: Mutex::new(false),
            wake: Condvar::new(),
            entered: Semaphore::new(0),
        })
    }
    fn wait(&self) {
        self.entered.add_permits(1);
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.wake.wait(released).unwrap();
        }
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
    async fn entered(&self) {
        timeout(Duration::from_secs(2), self.entered.acquire())
            .await
            .unwrap()
            .unwrap()
            .forget();
    }
}
struct ReleaseOnDrop(Arc<Gate>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct Factory {
    inner: TestSinkFactory,
    gate: Arc<Gate>,
    operation: Operation,
}
impl SinkFactory for Factory {
    fn id(&self) -> &StageId {
        self.inner.id()
    }
    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        if self.operation == Operation::DrainIncompatible {
            return Err(FactoryError::CreateFailed {
                message: "route cannot be reused".to_owned(),
            });
        }
        self.inner.compatibility_key(format)
    }
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        Ok(Box::new(Sink {
            inner: self.inner.create()?,
            gate: Arc::clone(&self.gate),
            operation: self.operation,
        }))
    }
}
struct Sink {
    inner: Box<dyn SinkStage>,
    gate: Arc<Gate>,
    operation: Operation,
}
impl SinkStage for Sink {
    fn open(&mut self, format: PcmFormat) -> Result<(), SinkError> {
        if self.operation == Operation::Open {
            self.gate.wait();
        }
        self.inner.open(format)
    }
    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        self.inner.write(block)
    }
    fn pause(&mut self) -> Result<(), SinkError> {
        self.inner.pause()
    }
    fn resume(&mut self) -> Result<(), SinkError> {
        if self.operation == Operation::Resume {
            self.gate.wait();
        }
        self.inner.resume()
    }
    fn drain(&mut self) -> Result<(), SinkError> {
        if matches!(
            self.operation,
            Operation::Drain | Operation::DrainIncompatible
        ) {
            self.gate.wait();
        }
        self.inner.drain()
    }
    fn discard(&mut self) -> Result<(), SinkError> {
        self.inner.discard()
    }
    fn clock_snapshot(&self) -> SinkClockSnapshot {
        self.inner.clock_snapshot()
    }
    fn close(&mut self) {
        if self.operation == Operation::Close {
            self.gate.wait();
        }
        self.inner.close();
    }
}
fn runtime(
    operation: Operation,
    gate: Arc<Gate>,
    samples: Arc<Mutex<Vec<f32>>>,
) -> PlaybackRuntime {
    let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(Factory {
            inner: TestSinkFactory {
                id: StageId::new("test.gated").unwrap(),
                samples,
            },
            gate,
            operation,
        }),
    });
    config.block_frames = 10;
    config.pcm_ring_blocks = 2;
    PlaybackRuntime::start(config).unwrap()
}

#[tokio::test]
async fn blocked_device_open_does_not_block_snapshot_or_stop_and_stale_activation_closes() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let runtime = runtime(
        Operation::Open,
        Arc::clone(&gate),
        Arc::new(Mutex::new(vec![])),
    );
    let controller = runtime.controller();
    let switch = tokio::spawn({
        let controller = controller.clone();
        async move {
            controller
                .switch_to(item(1, 100, 100), SwitchOptions::default())
                .await
        }
    });
    gate.entered().await;
    assert!(
        !switch.is_finished(),
        "activation must wait for actual device open"
    );
    timeout(Duration::from_millis(200), controller.snapshot())
        .await
        .unwrap()
        .unwrap();
    timeout(Duration::from_millis(200), controller.stop())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        timeout(Duration::from_millis(200), switch)
            .await
            .unwrap()
            .unwrap(),
        Err(PlaybackControlError::Closed)
    ));
    gate.release();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn blocked_device_control_defers_its_reply_without_blocking_later_actor_commands() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let runtime = runtime(
        Operation::Resume,
        Arc::clone(&gate),
        Arc::new(Mutex::new(vec![])),
    );
    let controller = runtime.controller();
    controller
        .switch_to(
            item(1, 100, 100),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();
    let play = tokio::spawn({
        let controller = controller.clone();
        async move { controller.play().await }
    });
    gate.entered().await;
    assert!(!play.is_finished());
    timeout(Duration::from_millis(200), controller.snapshot())
        .await
        .unwrap()
        .unwrap();
    timeout(Duration::from_millis(200), controller.stop())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        timeout(Duration::from_millis(200), play)
            .await
            .unwrap()
            .unwrap(),
        Err(PlaybackControlError::Closed)
    ));
    gate.release();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn blocked_drain_keeps_the_track_alive_and_only_ends_after_device_acknowledgement() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let samples = Arc::new(Mutex::new(vec![]));
    let runtime = runtime(Operation::Drain, Arc::clone(&gate), Arc::clone(&samples));
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch_to(item(1, 40, 100), SwitchOptions::default())
        .await
        .unwrap();
    gate.entered().await;
    let snapshot = timeout(Duration::from_millis(200), controller.snapshot())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.current_item_id, Some(item(1, 40, 100).id));
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, PlaybackEvent::PlaybackEnded { .. }));
    }
    gate.release();
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
    assert_eq!(samples.lock().unwrap().len(), 40);
}

#[tokio::test]
async fn stop_does_not_join_a_blocked_close_but_runtime_shutdown_waits_for_release() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let runtime = runtime(
        Operation::Close,
        Arc::clone(&gate),
        Arc::new(Mutex::new(vec![])),
    );
    let controller = runtime.controller();
    controller
        .switch_to(
            item(1, 100, 100),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();
    timeout(Duration::from_millis(200), controller.stop())
        .await
        .unwrap()
        .unwrap();
    gate.entered().await;
    assert_eq!(
        controller.snapshot().await.unwrap().state,
        PlaybackState::Idle
    );
    let shutdown = tokio::spawn(runtime.shutdown());
    tokio::task::yield_now().await;
    assert!(!shutdown.is_finished());
    gate.release();
    timeout(Duration::from_secs(2), shutdown)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn incompatible_successor_waits_for_old_output_drain_before_activation() {
    let gate = Gate::new();
    let _release = ReleaseOnDrop(Arc::clone(&gate));
    let samples = Arc::new(Mutex::new(vec![]));
    let runtime = runtime(
        Operation::DrainIncompatible,
        Arc::clone(&gate),
        Arc::clone(&samples),
    );
    let controller = runtime.controller();
    let mut events = controller.subscribe_events();
    controller
        .switch_to(
            item(1, 40, 100),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();
    controller.set_next(Some(item(2, 40, 50))).await.unwrap();
    controller.play().await.unwrap();
    gate.entered().await;
    assert_eq!(
        controller.snapshot().await.unwrap().current_item_id,
        Some(item(1, 40, 100).id)
    );
    gate.release();
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
    assert_eq!(samples.lock().unwrap().len(), 80);
}
