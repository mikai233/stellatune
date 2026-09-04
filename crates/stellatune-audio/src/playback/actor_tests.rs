use std::sync::{Arc, Mutex};
use std::thread::ThreadId;
use std::time::Duration;

use lattice_actor::context::HandlerContext;
use lattice_actor::error::ActorError;
use lattice_actor::error::{ActorCallError, ActorTellError};
use lattice_actor::reply::ReplyTo;
use lattice_actor::state_machine::Accepts;
use lattice_actor::traits::{Responder, StopReason};
use stellatune_audio_core::{error::PlaybackControlError, stage::StageId};
use tokio::sync::Semaphore;

use crate::planner::{StageRegistrySnapshot, TransitionPolicy};

use super::actor::{GetSnapshot, Play, PlaybackActor, PumpAudio};
use super::control::SwitchOptions;
use super::event::PlaybackState;
use super::runtime::{PlaybackRuntime, PlaybackRuntimeConfig};
use super::tests::support::{
    TestDecoderFactory, TestSinkFactory, delayed_item, runtime, signaled_delayed_item, wait_for_end,
};

#[derive(lattice_actor::Request)]
#[request(response = ThreadId)]
struct GetActorThread;

impl Accepts<GetActorThread> for PlaybackState {
    const ALWAYS: bool = true;

    fn accepts(&self) -> bool {
        true
    }
}

impl Responder<GetActorThread> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _request: GetActorThread,
        reply_to: ReplyTo<ThreadId>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(std::thread::current().id());
        Ok(())
    }
}

#[tokio::test]
async fn production_actor_uses_typed_behavior_admission_and_lifecycle() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(Vec::new())));
    let controller = runtime.controller();
    let actor = controller.actor.clone();

    let snapshot = actor
        .ask(GetSnapshot, Duration::from_secs(2))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.state, PlaybackState::Idle);
    assert!(matches!(
        actor.ask(Play, Duration::from_secs(2)).await,
        Err(ActorCallError::UnhandledInCurrentState)
    ));

    let mut terminated = actor.subscribe_terminated();
    actor.stop(StopReason::Requested).unwrap();
    tokio::time::timeout(Duration::from_secs(2), terminated.recv())
        .await
        .expect("production playback actor should stop")
        .expect("termination subscription should remain open");
}

#[tokio::test]
async fn production_actor_mailbox_is_bounded() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(Vec::new())));
    let controller = runtime.controller();
    let actor = controller.actor.clone();
    let mut events = controller.subscribe_events();
    let mut observed_full = false;
    for _ in 0..100_000 {
        if matches!(
            actor.try_tell(PumpAudio),
            Err(ActorTellError::MailboxFull(_))
        ) {
            observed_full = true;
            break;
        }
    }
    assert!(
        observed_full,
        "bounded production mailbox should apply backpressure"
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match controller
                .switch(
                    delayed_item(1, 10, 10, Duration::ZERO),
                    SwitchOptions::default(),
                )
                .await
            {
                Ok(()) => break,
                Err(PlaybackControlError::Failed(failure))
                    if failure.message.contains("mailbox is full") =>
                {
                    tokio::task::yield_now().await;
                },
                Err(error) => panic!("unexpected switch failure after saturation: {error}"),
            }
        }
    })
    .await
    .expect("mailbox should recover after a saturated tick burst");
    wait_for_end(&mut events).await;
    runtime.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_actor_stays_on_its_dedicated_execution_thread() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(Vec::new())));
    let actor = runtime.controller().actor.clone();
    let expected = actor
        .ask(GetActorThread, Duration::from_secs(2))
        .await
        .unwrap();

    for _ in 0..32 {
        tokio::task::yield_now().await;
        assert_eq!(
            actor
                .ask(GetActorThread, Duration::from_secs(2))
                .await
                .unwrap(),
            expected
        );
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn slow_preparation_keeps_active_session_controls_responsive() {
    let runtime = runtime(TransitionPolicy::Gapless, Arc::new(Mutex::new(Vec::new())));
    let controller = runtime.controller();
    controller
        .switch(
            delayed_item(1, 100, 10, Duration::ZERO),
            SwitchOptions {
                autoplay: false,
                ..SwitchOptions::default()
            },
        )
        .await
        .unwrap();

    let entered = Arc::new(Semaphore::new(0));
    let queued = {
        let controller = controller.clone();
        let entered = Arc::clone(&entered);
        tokio::spawn(async move {
            controller
                .queue_next(signaled_delayed_item(
                    2,
                    10,
                    5,
                    Duration::from_secs(30),
                    entered,
                ))
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(2), entered.acquire())
        .await
        .expect("deferred source preparation should start")
        .expect("preparation signal should remain open")
        .forget();

    assert_eq!(
        controller.snapshot().await.unwrap().state,
        PlaybackState::Ready
    );
    controller.pause().await.unwrap();
    controller.stop().await.unwrap();
    assert_eq!(queued.await.unwrap(), Err(PlaybackControlError::Closed));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn production_preparation_deadline_is_reported_and_cleans_state() {
    let registry = StageRegistrySnapshot {
        decoders: vec![Arc::new(TestDecoderFactory::new())],
        transforms: Vec::new(),
        sink: Arc::new(TestSinkFactory {
            id: StageId::new("test.deadline-sink").unwrap(),
            samples: Arc::new(Mutex::new(Vec::new())),
        }),
    };
    let mut config = PlaybackRuntimeConfig::new(registry);
    config.command_timeouts.preparation = Duration::from_millis(20);
    let runtime = PlaybackRuntime::start(config).unwrap();
    let controller = runtime.controller();

    assert_eq!(
        controller
            .switch(
                delayed_item(1, 10, 10, Duration::from_secs(1)),
                SwitchOptions::default(),
            )
            .await,
        Err(PlaybackControlError::CommandTimeout {
            operation: "switch"
        })
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if controller.snapshot().await.unwrap().state == PlaybackState::Failed {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("deadline cleanup should move the actor out of Preparing");
    runtime.shutdown().await.unwrap();
}
