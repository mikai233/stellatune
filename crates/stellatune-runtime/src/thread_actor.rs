use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::Sender;
use tokio::sync::oneshot;

pub trait Actor: Send + 'static {}

impl<T> Actor for T where T: Send + 'static {}

pub trait Message: Send + 'static {
    type Response: Send + 'static;
}

pub struct ActorContext<A: Actor> {
    stop_requested: bool,
    self_ref: Option<ActorRef<A>>,
    _marker: PhantomData<fn() -> A>,
}

impl<A: Actor> ActorContext<A> {
    fn new() -> Self {
        Self {
            stop_requested: false,
            self_ref: None,
            _marker: PhantomData,
        }
    }

    pub fn stop(&mut self) {
        self.stop_requested = true;
    }

    pub fn is_stop_requested(&self) -> bool {
        self.stop_requested
    }

    pub fn actor_ref(&self) -> ActorRef<A> {
        self.self_ref
            .as_ref()
            .expect("actor_ref is only available while handling a message")
            .clone()
    }

    fn enter_message(&mut self, self_ref: ActorRef<A>) {
        self.self_ref = Some(self_ref);
    }

    fn leave_message(&mut self) {
        self.self_ref = None;
    }
}

pub trait Handler<M>: Actor + Sized
where
    M: Message,
{
    fn handle(&mut self, message: M, ctx: &mut ActorContext<Self>) -> M::Response;
}

trait Envelope<A: Actor>: Send + 'static {
    fn handle(self: Box<Self>, actor: &mut A, ctx: &mut ActorContext<A>);
}

struct CastEnvelope<M, A>
where
    M: Message<Response = ()>,
    A: Handler<M>,
{
    message: M,
    self_ref: ActorRef<A>,
    _marker: PhantomData<fn() -> A>,
}

impl<M, A> Envelope<A> for CastEnvelope<M, A>
where
    M: Message<Response = ()>,
    A: Handler<M>,
{
    fn handle(self: Box<Self>, actor: &mut A, ctx: &mut ActorContext<A>) {
        ctx.enter_message(self.self_ref.clone());
        actor.handle(self.message, ctx);
        ctx.leave_message();
    }
}

struct CallEnvelope<M, A>
where
    M: Message,
    A: Handler<M>,
{
    message: M,
    response_tx: oneshot::Sender<M::Response>,
    self_ref: ActorRef<A>,
    _marker: PhantomData<fn() -> A>,
}

impl<M, A> Envelope<A> for CallEnvelope<M, A>
where
    M: Message,
    A: Handler<M>,
{
    fn handle(self: Box<Self>, actor: &mut A, ctx: &mut ActorContext<A>) {
        ctx.enter_message(self.self_ref.clone());
        let response = actor.handle(self.message, ctx);
        ctx.leave_message();
        let _ = self.response_tx.send(response);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastError {
    MailboxClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallError {
    MailboxClosed,
    Timeout,
    ActorStopped,
}

pub struct ActorRef<A: Actor> {
    tx: Sender<Box<dyn Envelope<A>>>,
}

impl<A: Actor> Clone for ActorRef<A> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl<A: Actor> ActorRef<A> {
    pub fn cast<M>(&self, message: M) -> Result<(), CastError>
    where
        M: Message<Response = ()>,
        A: Handler<M>,
    {
        let envelope: Box<dyn Envelope<A>> = Box::new(CastEnvelope::<M, A> {
            message,
            self_ref: self.clone(),
            _marker: PhantomData,
        });
        self.tx.send(envelope).map_err(|_| CastError::MailboxClosed)
    }

    pub fn call<M>(&self, message: M, timeout: Duration) -> Result<M::Response, CallError>
    where
        M: Message,
        A: Handler<M>,
    {
        crate::block_on(self.call_async(message, timeout))
    }

    pub async fn call_async<M>(
        &self,
        message: M,
        timeout: Duration,
    ) -> Result<M::Response, CallError>
    where
        M: Message,
        A: Handler<M>,
    {
        let response_rx = self.send_call(message)?;
        match tokio::time::timeout(timeout, response_rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(CallError::ActorStopped),
            Err(_) => Err(CallError::Timeout),
        }
    }

    fn send_call<M>(&self, message: M) -> Result<oneshot::Receiver<M::Response>, CallError>
    where
        M: Message,
        A: Handler<M>,
    {
        let (response_tx, response_rx) = oneshot::channel();
        let envelope: Box<dyn Envelope<A>> = Box::new(CallEnvelope::<M, A> {
            message,
            response_tx,
            self_ref: self.clone(),
            _marker: PhantomData,
        });
        self.tx
            .send(envelope)
            .map_err(|_| CallError::MailboxClosed)?;
        Ok(response_rx)
    }
}

pub fn spawn_actor<A: Actor>(actor: A) -> std::io::Result<(ActorRef<A>, JoinHandle<()>)> {
    spawn_actor_named(actor, "stellatune-thread-actor")
}

pub fn spawn_actor_named<A: Actor>(
    actor: A,
    thread_name: impl Into<String>,
) -> std::io::Result<(ActorRef<A>, JoinHandle<()>)> {
    let (tx, rx) = crossbeam_channel::unbounded::<Box<dyn Envelope<A>>>();
    let actor_ref = ActorRef { tx };
    let join = thread::Builder::new()
        .name(thread_name.into())
        .spawn(move || run_actor_loop(actor, rx))?;
    Ok((actor_ref, join))
}

fn run_actor_loop<A: Actor>(mut actor: A, rx: crossbeam_channel::Receiver<Box<dyn Envelope<A>>>) {
    let mut ctx = ActorContext::<A>::new();
    while let Ok(envelope) = rx.recv() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            envelope.handle(&mut actor, &mut ctx);
        }));
        if result.is_err() {
            break;
        }
        if ctx.is_stop_requested() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ActorContext, CallError, Handler, Message, spawn_actor};

    #[derive(Default)]
    struct CounterActor {
        value: u64,
    }

    struct Inc;
    impl Message for Inc {
        type Response = ();
    }

    struct Get;
    impl Message for Get {
        type Response = u64;
    }
    struct KickSelf;
    impl Message for KickSelf {
        type Response = ();
    }

    impl Handler<Inc> for CounterActor {
        fn handle(&mut self, _message: Inc, _ctx: &mut ActorContext<Self>) -> () {
            self.value = self.value.saturating_add(1);
        }
    }

    impl Handler<Get> for CounterActor {
        fn handle(&mut self, _message: Get, _ctx: &mut ActorContext<Self>) -> u64 {
            self.value
        }
    }

    impl Handler<KickSelf> for CounterActor {
        fn handle(&mut self, _message: KickSelf, ctx: &mut ActorContext<Self>) -> () {
            ctx.actor_ref().cast(Inc).expect("self cast");
        }
    }

    #[test]
    fn thread_actor_cast_and_call_work() {
        let (actor_ref, join) = spawn_actor(CounterActor::default()).expect("spawn actor");
        actor_ref.cast(Inc).expect("cast inc");
        let value = actor_ref
            .call(Get, Duration::from_millis(200))
            .expect("call get");
        assert_eq!(value, 1);
        drop(actor_ref);
        join.join().expect("join actor thread");
    }

    #[test]
    fn thread_actor_call_timeout() {
        #[derive(Default)]
        struct SlowActor;

        struct SlowCall;
        impl Message for SlowCall {
            type Response = u8;
        }

        impl Handler<SlowCall> for SlowActor {
            fn handle(&mut self, _message: SlowCall, _ctx: &mut ActorContext<Self>) -> u8 {
                std::thread::sleep(Duration::from_millis(80));
                7
            }
        }

        let (actor_ref, join) = spawn_actor(SlowActor).expect("spawn actor");
        let err = actor_ref
            .call(SlowCall, Duration::from_millis(10))
            .expect_err("expected timeout");
        assert_eq!(err, CallError::Timeout);
        drop(actor_ref);
        join.join().expect("join actor thread");
    }

    #[test]
    fn thread_actor_call_async_works() {
        crate::block_on(async {
            let (actor_ref, join) = spawn_actor(CounterActor::default()).expect("spawn actor");
            actor_ref.cast(Inc).expect("cast inc");
            let value = actor_ref
                .call_async(Get, Duration::from_millis(200))
                .await
                .expect("call async get");
            assert_eq!(value, 1);
            drop(actor_ref);
            join.join().expect("join actor thread");
        });
    }

    #[test]
    fn thread_actor_can_cast_to_self_from_context() {
        let (actor_ref, join) = spawn_actor(CounterActor::default()).expect("spawn actor");
        actor_ref
            .call(KickSelf, Duration::from_millis(200))
            .expect("kick self");
        let value = actor_ref
            .call(Get, Duration::from_millis(200))
            .expect("call get");
        assert_eq!(value, 1);
        drop(actor_ref);
        join.join().expect("join actor thread");
    }

    #[test]
    fn thread_actor_panic_is_isolated() {
        struct PanicCall;
        impl Message for PanicCall {
            type Response = u8;
        }

        impl Handler<PanicCall> for CounterActor {
            fn handle(&mut self, _message: PanicCall, _ctx: &mut ActorContext<Self>) -> u8 {
                panic!("panic in thread actor handler");
            }
        }

        let (actor_ref, join) = spawn_actor(CounterActor::default()).expect("spawn actor");
        let err = actor_ref
            .call(PanicCall, Duration::from_millis(200))
            .expect_err("panic call should fail");
        assert_eq!(err, CallError::ActorStopped);
        let next = actor_ref.call(Get, Duration::from_millis(200));
        assert!(matches!(
            next,
            Err(CallError::MailboxClosed) | Err(CallError::ActorStopped)
        ));
        drop(actor_ref);
        join.join().expect("join actor thread");
    }
}
