use std::{sync::Arc, time::Duration};

use lattice_actor::{
    actor_behavior,
    context::{ActorContext, HandlerContext},
    error::{ActorCallError, ActorError, ActorStopError, ActorTellError},
    mailbox::MailboxConfig,
    reply::ReplyTo,
    runtime::{ActorExecutionPolicy, ActorRuntime, ActorSpawnOptions},
    traits::{Actor, Handler, Responder, StopReason},
};
use tokio::sync::Semaphore;

const ASK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PlaybackState {
    #[default]
    Idle,
    Preparing,
    Ready,
    Playing,
    Paused,
    Reconfiguring {
        resume: ResumeState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeState {
    Ready,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlaybackSnapshot {
    state: PlaybackState,
    generation: u64,
    pumped_blocks: usize,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = u64)]
struct OpenTrack;

#[derive(Debug, lattice_actor::Message)]
struct SourceReady {
    generation: u64,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = PlaybackState)]
struct Play;

#[derive(Debug, lattice_actor::Request)]
#[request(response = PlaybackState)]
struct Pause;

#[derive(Debug, lattice_actor::Message)]
struct PumpAudio;

#[derive(Debug, lattice_actor::Request)]
#[request(response = PlaybackState)]
struct BeginReconfigure;

#[derive(Debug, lattice_actor::Request)]
#[request(response = PlaybackState)]
struct CompleteReconfigure;

#[derive(Debug, lattice_actor::Request)]
#[request(response = PlaybackSnapshot)]
struct Snapshot;

#[derive(Debug, lattice_actor::Request)]
#[request(response = String)]
struct CurrentThread;

#[derive(Debug, lattice_actor::Message)]
struct BlockTurn {
    entered: Arc<Semaphore>,
    release: Arc<Semaphore>,
}

#[derive(Debug, lattice_actor::Message)]
struct Noop;

actor_behavior! {
    PlaybackState {
        always => [Snapshot, CurrentThread, BlockTurn, Noop];
        PlaybackState::Idle => [OpenTrack];
        PlaybackState::Preparing => [SourceReady];
        PlaybackState::Ready => [Play, BeginReconfigure];
        PlaybackState::Playing => [Pause, PumpAudio, BeginReconfigure];
        PlaybackState::Paused => [Play, BeginReconfigure];
        PlaybackState::Reconfiguring { .. } => [CompleteReconfigure];
    }
}

struct PlaybackActorContract {
    generation: u64,
    pumped_blocks: usize,
    stopped: Arc<Semaphore>,
}

impl PlaybackActorContract {
    fn new(stopped: Arc<Semaphore>) -> Self {
        Self {
            generation: 0,
            pumped_blocks: 0,
            stopped,
        }
    }
}

impl Actor for PlaybackActorContract {
    type Error = ActorError;
    type Behavior = PlaybackState;

    async fn stopping(
        &mut self,
        _ctx: &mut ActorContext<Self>,
        _reason: StopReason,
    ) -> Result<(), ActorStopError> {
        self.stopped.add_permits(1);
        Ok(())
    }
}

impl Responder<OpenTrack> for PlaybackActorContract {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: OpenTrack,
        reply_to: ReplyTo<u64>,
    ) -> Result<(), ActorError> {
        self.generation = self.generation.wrapping_add(1);
        self.pumped_blocks = 0;
        ctx.transition_to(PlaybackState::Preparing);
        let _ = reply_to.send(self.generation);
        Ok(())
    }
}

impl Handler<SourceReady> for PlaybackActorContract {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: SourceReady,
    ) -> Result<(), ActorError> {
        if message.generation == self.generation {
            ctx.transition_to(PlaybackState::Ready);
        }
        Ok(())
    }
}

impl Responder<Play> for PlaybackActorContract {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: Play,
        reply_to: ReplyTo<PlaybackState>,
    ) -> Result<(), ActorError> {
        ctx.transition_to(PlaybackState::Playing);
        let _ = reply_to.send(PlaybackState::Playing);
        Ok(())
    }
}

impl Responder<Pause> for PlaybackActorContract {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: Pause,
        reply_to: ReplyTo<PlaybackState>,
    ) -> Result<(), ActorError> {
        ctx.transition_to(PlaybackState::Paused);
        let _ = reply_to.send(PlaybackState::Paused);
        Ok(())
    }
}

impl Handler<PumpAudio> for PlaybackActorContract {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: PumpAudio,
    ) -> Result<(), ActorError> {
        self.pumped_blocks += 1;
        Ok(())
    }
}

impl Responder<BeginReconfigure> for PlaybackActorContract {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: BeginReconfigure,
        reply_to: ReplyTo<PlaybackState>,
    ) -> Result<(), ActorError> {
        let resume = match ctx.behavior() {
            PlaybackState::Ready => ResumeState::Ready,
            PlaybackState::Playing => ResumeState::Playing,
            PlaybackState::Paused => ResumeState::Paused,
            _ => unreachable!("behavior admission restricts reconfiguration states"),
        };
        let state = PlaybackState::Reconfiguring { resume };
        ctx.transition_to(state);
        let _ = reply_to.send(state);
        Ok(())
    }
}

impl Responder<CompleteReconfigure> for PlaybackActorContract {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: CompleteReconfigure,
        reply_to: ReplyTo<PlaybackState>,
    ) -> Result<(), ActorError> {
        let PlaybackState::Reconfiguring { resume } = ctx.behavior() else {
            unreachable!("behavior admission restricts completion to reconfiguration")
        };
        let state = match resume {
            ResumeState::Ready => PlaybackState::Ready,
            ResumeState::Playing => PlaybackState::Playing,
            ResumeState::Paused => PlaybackState::Paused,
        };
        ctx.transition_to(state);
        let _ = reply_to.send(state);
        Ok(())
    }
}

impl Responder<Snapshot> for PlaybackActorContract {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: Snapshot,
        reply_to: ReplyTo<PlaybackSnapshot>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(PlaybackSnapshot {
            state: *ctx.behavior(),
            generation: self.generation,
            pumped_blocks: self.pumped_blocks,
        });
        Ok(())
    }
}

impl Responder<CurrentThread> for PlaybackActorContract {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _request: CurrentThread,
        reply_to: ReplyTo<String>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(format!("{:?}", std::thread::current().id()));
        Ok(())
    }
}

impl Handler<BlockTurn> for PlaybackActorContract {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: BlockTurn,
    ) -> Result<(), ActorError> {
        message.entered.add_permits(1);
        message
            .release
            .acquire()
            .await
            .expect("release semaphore should remain open")
            .forget();
        Ok(())
    }
}

impl Handler<Noop> for PlaybackActorContract {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: Noop,
    ) -> Result<(), ActorError> {
        Ok(())
    }
}

fn spawn_contract_actor(
    mailbox: MailboxConfig,
    stopped: Arc<Semaphore>,
) -> lattice_actor::handle::ActorHandle<PlaybackActorContract> {
    ActorRuntime::default()
        .spawn_actor(
            PlaybackActorContract::new(stopped),
            ActorSpawnOptions {
                mailbox,
                execution: Some(ActorExecutionPolicy::DedicatedThreadPool { worker_count: 1 }),
                ..ActorSpawnOptions::default()
            },
        )
        .expect("contract actor should spawn")
}

#[tokio::test]
async fn typed_playback_messages_follow_behavior_admission() {
    let stopped = Arc::new(Semaphore::new(0));
    let handle = spawn_contract_actor(MailboxConfig::bounded(8), Arc::clone(&stopped));

    assert_eq!(
        handle.ask(Snapshot, ASK_TIMEOUT).await.unwrap(),
        PlaybackSnapshot {
            state: PlaybackState::Idle,
            generation: 0,
            pumped_blocks: 0,
        }
    );
    assert!(matches!(
        handle.ask(Play, ASK_TIMEOUT).await,
        Err(ActorCallError::UnhandledInCurrentState)
    ));

    let generation = handle.ask(OpenTrack, ASK_TIMEOUT).await.unwrap();
    handle
        .tell(SourceReady {
            generation: generation.wrapping_sub(1),
        })
        .await
        .unwrap();
    assert_eq!(
        handle.ask(Snapshot, ASK_TIMEOUT).await.unwrap().state,
        PlaybackState::Preparing
    );

    handle.tell(SourceReady { generation }).await.unwrap();
    assert_eq!(
        handle.ask(Play, ASK_TIMEOUT).await.unwrap(),
        PlaybackState::Playing
    );
    handle.tell(PumpAudio).await.unwrap();
    assert_eq!(
        handle
            .ask(Snapshot, ASK_TIMEOUT)
            .await
            .unwrap()
            .pumped_blocks,
        1
    );

    assert_eq!(
        handle.ask(BeginReconfigure, ASK_TIMEOUT).await.unwrap(),
        PlaybackState::Reconfiguring {
            resume: ResumeState::Playing,
        }
    );
    assert!(matches!(
        handle.ask(Pause, ASK_TIMEOUT).await,
        Err(ActorCallError::UnhandledInCurrentState)
    ));
    assert_eq!(
        handle.ask(CompleteReconfigure, ASK_TIMEOUT).await.unwrap(),
        PlaybackState::Playing
    );
    assert_eq!(
        handle.ask(Pause, ASK_TIMEOUT).await.unwrap(),
        PlaybackState::Paused
    );

    let first_thread = handle.ask(CurrentThread, ASK_TIMEOUT).await.unwrap();
    let second_thread = handle.ask(CurrentThread, ASK_TIMEOUT).await.unwrap();
    assert_eq!(first_thread, second_thread);

    handle.stop(StopReason::Requested).unwrap();
    tokio::time::timeout(ASK_TIMEOUT, stopped.acquire())
        .await
        .expect("actor should stop before timeout")
        .expect("stop semaphore should remain open")
        .forget();
}

#[tokio::test]
async fn bounded_mailbox_reports_backpressure_without_losing_message_ownership() {
    let stopped = Arc::new(Semaphore::new(0));
    let entered = Arc::new(Semaphore::new(0));
    let release = Arc::new(Semaphore::new(0));
    let handle = spawn_contract_actor(MailboxConfig::bounded(1), Arc::clone(&stopped));

    handle
        .try_tell(BlockTurn {
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
        })
        .unwrap();
    tokio::time::timeout(ASK_TIMEOUT, entered.acquire())
        .await
        .expect("blocking turn should start")
        .expect("entered semaphore should remain open")
        .forget();

    handle.try_tell(Noop).unwrap();
    assert!(matches!(
        handle.try_tell(Noop),
        Err(ActorTellError::MailboxFull(Noop))
    ));

    release.add_permits(1);
    handle.stop(StopReason::Requested).unwrap();
    tokio::time::timeout(ASK_TIMEOUT, stopped.acquire())
        .await
        .expect("actor should stop after releasing the active turn")
        .expect("stop semaphore should remain open")
        .forget();
}
