use std::future::Future;
use std::marker::PhantomData;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::time::Duration;

use futures_util::FutureExt;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

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

#[async_trait::async_trait]
pub trait Handler<M>: Actor
where
    M: Message,
    Self: Sized,
{
    async fn handle(&mut self, message: M, ctx: &mut ActorContext<Self>) -> M::Response;
}

trait Envelope<A: Actor>: Send + 'static {
    fn handle<'a>(
        self: Box<Self>,
        actor: &'a mut A,
        ctx: &'a mut ActorContext<A>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
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
    fn handle<'a>(
        self: Box<Self>,
        actor: &'a mut A,
        ctx: &'a mut ActorContext<A>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            ctx.enter_message(self.self_ref.clone());
            actor.handle(self.message, ctx).await;
            ctx.leave_message();
        })
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
    fn handle<'a>(
        self: Box<Self>,
        actor: &'a mut A,
        ctx: &'a mut ActorContext<A>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            ctx.enter_message(self.self_ref.clone());
            let response = actor.handle(self.message, ctx).await;
            ctx.leave_message();
            let _ = self.response_tx.send(response);
        })
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
    tx: mpsc::UnboundedSender<Box<dyn Envelope<A>>>,
}

impl<A: Actor> Clone for ActorRef<A> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl<A: Actor> ActorRef<A> {
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

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

    pub async fn call<M>(&self, message: M, timeout: Duration) -> Result<M::Response, CallError>
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
        match tokio::time::timeout(timeout, response_rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(CallError::ActorStopped),
            Err(_) => Err(CallError::Timeout),
        }
    }
}

pub fn spawn_actor<A: Actor>(actor: A) -> (ActorRef<A>, JoinHandle<()>) {
    let (tx, rx) = mpsc::unbounded_channel::<Box<dyn Envelope<A>>>();
    let actor_ref = ActorRef { tx };
    let join = crate::spawn(run_actor_loop(actor, rx));
    (actor_ref, join)
}

async fn run_actor_loop<A: Actor>(
    mut actor: A,
    mut rx: mpsc::UnboundedReceiver<Box<dyn Envelope<A>>>,
) {
    let mut ctx = ActorContext::<A>::new();
    while let Some(envelope) = rx.recv().await {
        let result = AssertUnwindSafe(envelope.handle(&mut actor, &mut ctx))
            .catch_unwind()
            .await;
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

    #[async_trait::async_trait]
    impl Handler<Inc> for CounterActor {
        async fn handle(&mut self, _message: Inc, _ctx: &mut ActorContext<Self>) -> () {
            self.value = self.value.saturating_add(1);
        }
    }

    #[async_trait::async_trait]
    impl Handler<Get> for CounterActor {
        async fn handle(&mut self, _message: Get, _ctx: &mut ActorContext<Self>) -> u64 {
            self.value
        }
    }

    #[async_trait::async_trait]
    impl Handler<KickSelf> for CounterActor {
        async fn handle(&mut self, _message: KickSelf, ctx: &mut ActorContext<Self>) -> () {
            ctx.actor_ref().cast(Inc).expect("self cast");
        }
    }

    #[test]
    fn tokio_actor_cast_and_call_work() {
        crate::block_on(async {
            let (actor_ref, join) = spawn_actor(CounterActor::default());
            actor_ref.cast(Inc).expect("cast inc");
            let value = actor_ref
                .call(Get, Duration::from_millis(200))
                .await
                .expect("call get");
            assert_eq!(value, 1);
            drop(actor_ref);
            join.await.expect("join actor task");
        });
    }

    #[test]
    fn tokio_actor_call_timeout() {
        struct SlowActor;

        struct SlowCall;
        impl Message for SlowCall {
            type Response = u8;
        }

        #[async_trait::async_trait]
        impl Handler<SlowCall> for SlowActor {
            async fn handle(&mut self, _message: SlowCall, _ctx: &mut ActorContext<Self>) -> u8 {
                tokio::time::sleep(Duration::from_millis(80)).await;
                9
            }
        }

        crate::block_on(async {
            let (actor_ref, join) = spawn_actor(SlowActor);
            let err = actor_ref
                .call(SlowCall, Duration::from_millis(10))
                .await
                .expect_err("expected timeout");
            assert_eq!(err, CallError::Timeout);
            drop(actor_ref);
            join.await.expect("join actor task");
        });
    }

    #[test]
    fn tokio_actor_can_cast_to_self_from_context() {
        crate::block_on(async {
            let (actor_ref, join) = spawn_actor(CounterActor::default());
            actor_ref
                .call(KickSelf, Duration::from_millis(200))
                .await
                .expect("kick self");
            let value = actor_ref
                .call(Get, Duration::from_millis(200))
                .await
                .expect("call get");
            assert_eq!(value, 1);
            drop(actor_ref);
            join.await.expect("join actor task");
        });
    }

    #[test]
    fn tokio_actor_panic_is_isolated() {
        struct PanicCall;
        impl Message for PanicCall {
            type Response = u8;
        }

        #[async_trait::async_trait]
        impl Handler<PanicCall> for CounterActor {
            async fn handle(&mut self, _message: PanicCall, _ctx: &mut ActorContext<Self>) -> u8 {
                panic!("panic in tokio actor handler");
            }
        }

        crate::block_on(async {
            let (actor_ref, join) = spawn_actor(CounterActor::default());
            let err = actor_ref
                .call(PanicCall, Duration::from_millis(200))
                .await
                .expect_err("panic call should fail");
            assert_eq!(err, CallError::ActorStopped);
            let next = actor_ref.call(Get, Duration::from_millis(200)).await;
            assert!(matches!(
                next,
                Err(CallError::MailboxClosed) | Err(CallError::ActorStopped)
            ));
            drop(actor_ref);
            join.await.expect("join actor task");
        });
    }
}
