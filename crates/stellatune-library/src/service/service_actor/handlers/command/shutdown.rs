use super::{ActorContext, Handler, LibraryServiceActor, Message};

pub(crate) struct ShutdownMessage;

impl Message for ShutdownMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<ShutdownMessage> for LibraryServiceActor {
    async fn handle(&mut self, _message: ShutdownMessage, ctx: &mut ActorContext<Self>) -> () {
        tracing::info!("library actor exiting");
        ctx.stop();
    }
}
