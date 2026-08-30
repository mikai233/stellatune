use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use lattice_actor::{
    context::{ActorContext, HandlerContext},
    error::{ActorCallError, ActorError, ActorStopError},
    handle::ActorHandle,
    mailbox::MailboxConfig,
    reply::ReplyTo,
    runtime::spawn_actor,
    state_machine::Stateless,
    traits::{
        Actor, ChildActorKey, ChildActorOptions, ChildSupervision, Handler, Responder, StopReason,
    },
};
use tokio::sync::{Mutex, Semaphore};

const ASK_TIMEOUT: Duration = Duration::from_secs(2);

struct RuntimeContractActor {
    events: Arc<Mutex<Vec<&'static str>>>,
    stopped: Arc<Semaphore>,
}

impl Actor for RuntimeContractActor {
    type Error = ActorError;
    type Behavior = Stateless;

    async fn stopping(
        &mut self,
        _ctx: &mut ActorContext<Self>,
        _reason: StopReason,
    ) -> Result<(), ActorStopError> {
        self.stopped.add_permits(1);
        Ok(())
    }
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = usize)]
struct DeferredLoad {
    gate: Arc<Semaphore>,
    entered: Arc<Semaphore>,
    value: usize,
}

#[derive(lattice_actor::Message)]
struct DeferredReady {
    value: usize,
    reply_to: ReplyTo<usize>,
}

#[derive(Debug, lattice_actor::Message)]
struct StartPipe {
    gate: Arc<Semaphore>,
    entered: Arc<Semaphore>,
    completed: Arc<Semaphore>,
}

#[derive(Debug, lattice_actor::Message)]
struct PipeReady {
    completed: Arc<Semaphore>,
}

#[derive(Debug, lattice_actor::Message)]
struct Record {
    value: &'static str,
    processed: Arc<Semaphore>,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = usize)]
struct EventCount;

impl Responder<DeferredLoad> for RuntimeContractActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        request: DeferredLoad,
        reply_to: ReplyTo<usize>,
    ) -> Result<(), ActorError> {
        request.entered.add_permits(1);
        ctx.defer_reply(
            reply_to,
            async move {
                request
                    .gate
                    .acquire_owned()
                    .await
                    .expect("deferred gate should remain open")
                    .forget();
                request.value
            },
            |value, reply_to| DeferredReady { value, reply_to },
        )?;
        Ok(())
    }
}

impl Handler<DeferredReady> for RuntimeContractActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: DeferredReady,
    ) -> Result<(), ActorError> {
        self.events.lock().await.push("deferred");
        message.reply_to.send(message.value)?;
        Ok(())
    }
}

impl Handler<StartPipe> for RuntimeContractActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: StartPipe,
    ) -> Result<(), ActorError> {
        message.entered.add_permits(1);
        let completed = message.completed;
        ctx.pipe_to_self(
            async move {
                message
                    .gate
                    .acquire_owned()
                    .await
                    .expect("pipe gate should remain open")
                    .forget();
            },
            move |()| PipeReady { completed },
        )?;
        Ok(())
    }
}

impl Handler<PipeReady> for RuntimeContractActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: PipeReady,
    ) -> Result<(), ActorError> {
        self.events.lock().await.push("piped");
        message.completed.add_permits(1);
        Ok(())
    }
}

impl Handler<Record> for RuntimeContractActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: Record,
    ) -> Result<(), ActorError> {
        self.events.lock().await.push(message.value);
        message.processed.add_permits(1);
        Ok(())
    }
}

impl Responder<EventCount> for RuntimeContractActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _request: EventCount,
        reply_to: ReplyTo<usize>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(self.events.lock().await.len());
        Ok(())
    }
}

type RuntimeActorFixture = (
    ActorHandle<RuntimeContractActor>,
    Arc<Mutex<Vec<&'static str>>>,
    Arc<Semaphore>,
);

fn spawn_runtime_actor(deferred_capacity: usize) -> RuntimeActorFixture {
    let events = Arc::new(Mutex::new(Vec::new()));
    let stopped = Arc::new(Semaphore::new(0));
    let handle = spawn_actor(
        RuntimeContractActor {
            events: Arc::clone(&events),
            stopped: Arc::clone(&stopped),
        },
        MailboxConfig::bounded(8).with_deferred_capacity(deferred_capacity),
    );
    (handle, events, stopped)
}

#[tokio::test]
async fn deferred_reply_preserves_mailbox_responsiveness_and_capacity() {
    let (handle, events, stopped) = spawn_runtime_actor(1);
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(Semaphore::new(0));
    let pending = tokio::spawn({
        let handle = handle.clone();
        let gate = Arc::clone(&gate);
        let entered = Arc::clone(&entered);
        async move {
            handle
                .ask(
                    DeferredLoad {
                        gate,
                        entered,
                        value: 7,
                    },
                    ASK_TIMEOUT,
                )
                .await
        }
    });

    entered
        .acquire()
        .await
        .expect("deferred request should enter")
        .forget();
    let control_processed = Arc::new(Semaphore::new(0));
    handle
        .tell(Record {
            value: "control",
            processed: Arc::clone(&control_processed),
        })
        .await
        .unwrap();
    tokio::time::timeout(ASK_TIMEOUT, control_processed.acquire())
        .await
        .expect("control message should not wait for deferred work")
        .expect("control signal should remain open")
        .forget();

    let saturated = handle
        .ask(
            DeferredLoad {
                gate: Arc::new(Semaphore::new(0)),
                entered: Arc::new(Semaphore::new(0)),
                value: 8,
            },
            ASK_TIMEOUT,
        )
        .await;
    assert!(matches!(saturated, Err(ActorCallError::MailboxFull)));

    gate.add_permits(1);
    assert_eq!(pending.await.unwrap().unwrap(), 7);
    assert_eq!(*events.lock().await, vec!["control", "deferred"]);

    handle.stop(StopReason::Requested).unwrap();
    tokio::time::timeout(ASK_TIMEOUT, stopped.acquire())
        .await
        .expect("runtime actor should stop")
        .expect("stop signal should remain open")
        .forget();
}

#[tokio::test]
async fn deadline_cancels_deferred_work_and_discards_late_result() {
    let (handle, events, _stopped) = spawn_runtime_actor(1);
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(Semaphore::new(0));

    let result = handle
        .ask(
            DeferredLoad {
                gate: Arc::clone(&gate),
                entered: Arc::clone(&entered),
                value: 11,
            },
            Duration::from_millis(30),
        )
        .await;
    assert!(matches!(result, Err(ActorCallError::DeadlineExceeded)));

    gate.add_permits(1);
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(events.lock().await.is_empty());
    assert_eq!(handle.ask(EventCount, ASK_TIMEOUT).await.unwrap(), 0);
}

#[tokio::test]
async fn pipe_to_self_returns_async_completion_through_the_mailbox() {
    let (handle, events, _stopped) = spawn_runtime_actor(2);
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(Semaphore::new(0));
    let completed = Arc::new(Semaphore::new(0));

    handle
        .tell(StartPipe {
            gate: Arc::clone(&gate),
            entered: Arc::clone(&entered),
            completed: Arc::clone(&completed),
        })
        .await
        .unwrap();
    entered.acquire().await.expect("pipe should start").forget();

    assert_eq!(handle.ask(EventCount, ASK_TIMEOUT).await.unwrap(), 0);
    gate.add_permits(1);
    tokio::time::timeout(ASK_TIMEOUT, completed.acquire())
        .await
        .expect("pipe completion should return")
        .expect("completion signal should remain open")
        .forget();
    assert_eq!(*events.lock().await, vec!["piped"]);
}

struct SupervisedChild;

impl Actor for SupervisedChild {
    type Error = ActorError;
    type Behavior = Stateless;
}

#[derive(Debug, lattice_actor::Message)]
struct RestartChild;

struct SupervisingActor {
    child: Option<ActorHandle<SupervisedChild>>,
    child_starts: Arc<AtomicUsize>,
    child_started: Arc<Semaphore>,
}

impl Actor for SupervisingActor {
    type Error = ActorError;
    type Behavior = Stateless;

    async fn started(&mut self, ctx: &mut ActorContext<Self>) -> Result<(), ActorError> {
        let child_starts = Arc::clone(&self.child_starts);
        let child_started = Arc::clone(&self.child_started);
        self.child = Some(ctx.spawn_child_with_factory(
            ChildActorKey::new("plugin-process"),
            move || {
                child_starts.fetch_add(1, Ordering::SeqCst);
                child_started.add_permits(1);
                SupervisedChild
            },
            ChildActorOptions {
                mailbox: MailboxConfig::bounded(8),
                supervision: ChildSupervision::RestartChild,
                ..ChildActorOptions::default()
            },
        )?);
        Ok(())
    }
}

impl Handler<RestartChild> for SupervisingActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: RestartChild,
    ) -> Result<(), ActorError> {
        self.child
            .as_ref()
            .expect("supervised child should exist")
            .stop(StopReason::Requested)
            .map_err(|error| ActorError::new(error.to_string()))?;
        Ok(())
    }
}

#[tokio::test]
async fn child_supervision_restarts_factory_and_parent_shutdown_is_observable() {
    let child_starts = Arc::new(AtomicUsize::new(0));
    let child_started = Arc::new(Semaphore::new(0));
    let parent = spawn_actor(
        SupervisingActor {
            child: None,
            child_starts: Arc::clone(&child_starts),
            child_started: Arc::clone(&child_started),
        },
        MailboxConfig::bounded(8),
    );
    let mut terminated = parent.subscribe_terminated();

    tokio::time::timeout(ASK_TIMEOUT, child_started.acquire())
        .await
        .expect("child should start")
        .expect("child start signal should remain open")
        .forget();
    parent.tell(RestartChild).await.unwrap();
    tokio::time::timeout(ASK_TIMEOUT, child_started.acquire())
        .await
        .expect("supervisor should restart child")
        .expect("child start signal should remain open")
        .forget();
    assert_eq!(child_starts.load(Ordering::SeqCst), 2);

    parent.stop(StopReason::Requested).unwrap();
    tokio::time::timeout(ASK_TIMEOUT, terminated.recv())
        .await
        .expect("parent termination should be observable")
        .expect("termination subscription should remain open");
}
