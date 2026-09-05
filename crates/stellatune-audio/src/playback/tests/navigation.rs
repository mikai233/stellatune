use super::support::{item, runtime, signaled_delayed_item};

#[tokio::test]
async fn advancement_during_crossfade_reuses_the_secondary_and_then_its_successor() {
    use super::support::{TestDecoderFactory, TestSinkFactory};
    use crate::{
        planner::{CrossfadeCurve, CrossfadeFallback, StageRegistrySnapshot},
        playback::runtime::{PlaybackRuntime, PlaybackRuntimeConfig},
    };
    use stellatune_audio_core::stage::StageId;
    let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(TestSinkFactory {
            id: StageId::new("test.overlap").unwrap(),
            samples: Arc::new(Mutex::new(vec![])),
        }),
    });
    config.max_pcm_blocks = 2;
    config.policies.transition = TransitionPolicy::Crossfade {
        duration_frames: 200,
        curve: CrossfadeCurve::Linear,
        fallback: CrossfadeFallback::Gapless,
    };
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 255, 10), paused())
        .await
        .unwrap();
    let (second, opens) = counted(item(2, 255, 20));
    controller.set_next(Some(second)).await.unwrap();
    let mut events = controller.subscribe_events();
    controller.play().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if matches!(events.recv().await.unwrap(), PlaybackEvent::TrackChanged { item_id } if item_id == PlaybackItemId::new(2).unwrap()) { break; }
        }
    }).await.unwrap();
    controller.pause().await.unwrap();
    assert_eq!(
        controller
            .advance_to_next(PlaybackItemId::new(2).unwrap(), SwitchOptions::default())
            .await
            .unwrap(),
        AdvanceOutcome::Accepted
    );
    controller.set_next(Some(item(3, 255, 30))).await.unwrap();
    assert_eq!(
        controller
            .advance_to_next(PlaybackItemId::new(3).unwrap(), SwitchOptions::default())
            .await
            .unwrap(),
        AdvanceOutcome::Accepted
    );
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if matches!(events.recv().await.unwrap(), PlaybackEvent::TrackChanged { item_id } if item_id == PlaybackItemId::new(3).unwrap()) { break; }
        }
    }).await.unwrap();
    assert_eq!(opens.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn current_recovery_does_not_reopen_the_prepared_successor() {
    use super::support::{RecoveringSinkFactory, TestDecoderFactory};
    use crate::{
        planner::StageRegistrySnapshot,
        playback::runtime::{PlaybackRuntime, PlaybackRuntimeConfig},
    };
    use stellatune_audio_core::stage::StageId;
    let mut config = PlaybackRuntimeConfig::new(StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: vec![],
        sink: Arc::new(RecoveringSinkFactory {
            id: StageId::new("test.recovery").unwrap(),
            samples: Arc::new(Mutex::new(vec![])),
            creates: Arc::new(AtomicUsize::new(0)),
        }),
    });
    config.policies.recovery_backoff_ms = 0;
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 10), paused())
        .await
        .unwrap();
    let (second, opens) = counted(item(2, 100, 20));
    controller.set_next(Some(second)).await.unwrap();
    let mut events = controller.subscribe_events();
    controller.play().await.unwrap();
    let recovered = tokio::time::timeout(Duration::from_secs(3), async {
        let mut recovered = false;
        loop {
            match events.recv().await.unwrap() {
                PlaybackEvent::StateChanged(PlaybackState::Recovering) => recovered = true,
                PlaybackEvent::TrackChanged { item_id }
                    if item_id == PlaybackItemId::new(2).unwrap() =>
                {
                    break recovered;
                },
                _ => {},
            }
        }
    })
    .await
    .unwrap();
    assert!(recovered);
    assert_eq!(opens.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}
use crate::{
    planner::TransitionPolicy,
    playback::{
        control::{AdvanceOutcome, SwitchOptions, SwitchTransition},
        event::{PlaybackEvent, PlaybackState},
    },
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use stellatune_audio_core::{
    playback::{PlaybackItem, PlaybackItemId},
    source::{SourceDescriptor, SourceFactory, SourceOpenFuture, SourceOpenRequest},
};
use tokio::sync::Semaphore;

struct CountOpens {
    inner: Arc<dyn SourceFactory>,
    opens: Arc<AtomicUsize>,
}
impl SourceFactory for CountOpens {
    fn descriptor(&self) -> SourceDescriptor {
        self.inner.descriptor()
    }
    fn open(&self, request: SourceOpenRequest) -> SourceOpenFuture<'_> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        self.inner.open(request)
    }
}

fn counted(mut item: PlaybackItem) -> (PlaybackItem, Arc<AtomicUsize>) {
    let opens = Arc::new(AtomicUsize::new(0));
    item.source = Arc::new(CountOpens {
        inner: item.source,
        opens: Arc::clone(&opens),
    });
    (item, opens)
}

fn paused() -> SwitchOptions {
    SwitchOptions {
        autoplay: false,
        transition: SwitchTransition::ImmediateWithDeClick,
    }
}

#[tokio::test]
async fn ready_successor_is_claimed_without_reopening_and_preserves_paused_intent() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(vec![])));
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 1), paused())
        .await
        .unwrap();
    let (next, opens) = counted(item(2, 100, 2));
    let id = next.id;
    controller.set_next(Some(next)).await.unwrap();
    assert_eq!(
        controller.advance_to_next(id, paused()).await.unwrap(),
        AdvanceOutcome::Accepted
    );
    let snapshot = controller.snapshot().await.unwrap();
    assert_eq!(snapshot.current_item_id, Some(id));
    assert_eq!(snapshot.state, PlaybackState::Ready);
    assert_eq!(opens.load(Ordering::SeqCst), 1);
    // A delayed manual-next request must not reopen an item already promoted.
    assert_eq!(
        controller.advance_to_next(id, paused()).await.unwrap(),
        AdvanceOutcome::AlreadyCurrent
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn preparing_successor_is_claimed_without_cancellation_or_duplicate_open() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(vec![])));
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 1), paused())
        .await
        .unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let (next, opens) = counted(signaled_delayed_item(
        2,
        100,
        2,
        Duration::from_millis(50),
        Arc::clone(&entered),
    ));
    let id = next.id;
    let preparation = {
        let controller = controller.clone();
        tokio::spawn(async move { controller.set_next(Some(next)).await })
    };
    entered.acquire().await.unwrap().forget();
    assert_eq!(
        controller.advance_to_next(id, paused()).await.unwrap(),
        AdvanceOutcome::Accepted
    );
    assert_eq!(
        controller.snapshot().await.unwrap().current_item_id,
        PlaybackItemId::new(1)
    );
    preparation.await.unwrap().unwrap();
    assert_eq!(
        controller.snapshot().await.unwrap().current_item_id,
        Some(id)
    );
    assert_eq!(opens.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn identity_mismatch_does_not_consume_the_prepared_occurrence() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(vec![])));
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 1), paused())
        .await
        .unwrap();
    controller.set_next(Some(item(2, 100, 2))).await.unwrap();
    assert_eq!(
        controller
            .advance_to_next(PlaybackItemId::new(3).unwrap(), paused())
            .await
            .unwrap(),
        AdvanceOutcome::Unavailable
    );
    assert_eq!(
        controller
            .advance_to_next(PlaybackItemId::new(2).unwrap(), paused())
            .await
            .unwrap(),
        AdvanceOutcome::Accepted
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn pause_after_claiming_a_slow_successor_overrides_its_autoplay_intent() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(vec![])));
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 1), paused())
        .await
        .unwrap();
    let entered = Arc::new(Semaphore::new(0));
    let next = signaled_delayed_item(2, 100, 2, Duration::from_millis(50), Arc::clone(&entered));
    let preparation = {
        let controller = controller.clone();
        tokio::spawn(async move { controller.set_next(Some(next)).await })
    };
    entered.acquire().await.unwrap().forget();
    controller
        .advance_to_next(PlaybackItemId::new(2).unwrap(), SwitchOptions::default())
        .await
        .unwrap();
    controller.pause().await.unwrap();
    preparation.await.unwrap().unwrap();
    let snapshot = controller.snapshot().await.unwrap();
    assert_eq!(snapshot.current_item_id, PlaybackItemId::new(2));
    assert_eq!(snapshot.state, PlaybackState::Ready);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn replacing_a_claimed_preparation_never_activates_its_stale_result() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(vec![])));
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 1), paused())
        .await
        .unwrap();
    let mut events = controller.subscribe_events();
    let entered = Arc::new(Semaphore::new(0));
    let next = signaled_delayed_item(2, 100, 2, Duration::from_secs(30), Arc::clone(&entered));
    let preparation = {
        let controller = controller.clone();
        tokio::spawn(async move { controller.set_next(Some(next)).await })
    };
    entered.acquire().await.unwrap().forget();
    controller
        .advance_to_next(PlaybackItemId::new(2).unwrap(), paused())
        .await
        .unwrap();
    controller
        .switch_to(item(3, 100, 3), paused())
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(2), preparation)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(
        controller.snapshot().await.unwrap().current_item_id,
        PlaybackItemId::new(3)
    );
    while let Ok(event) = events.try_recv() {
        assert!(
            !matches!(event, PlaybackEvent::TrackChanged { item_id } if item_id == PlaybackItemId::new(2).unwrap())
        );
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn explicit_switch_to_prepared_identity_reuses_it_and_clear_removes_it() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(vec![])));
    let controller = runtime.controller();
    controller
        .switch_to(item(1, 100, 1), paused())
        .await
        .unwrap();
    let (next, opens) = counted(item(2, 100, 2));
    controller.set_next(Some(next.clone())).await.unwrap();
    controller.switch_to(next, paused()).await.unwrap();
    assert_eq!(opens.load(Ordering::SeqCst), 1);
    controller.set_next(Some(item(3, 100, 3))).await.unwrap();
    controller.set_next(None).await.unwrap();
    assert_eq!(
        controller
            .advance_to_next(PlaybackItemId::new(3).unwrap(), paused())
            .await
            .unwrap(),
        AdvanceOutcome::Unavailable
    );
    runtime.shutdown().await.unwrap();
}
