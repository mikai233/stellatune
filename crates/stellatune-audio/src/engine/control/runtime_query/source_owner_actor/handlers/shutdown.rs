use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::SourceOwnerActor;

pub(crate) struct SourceShutdownMessage;

impl Message for SourceShutdownMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<SourceShutdownMessage> for SourceOwnerActor {
    async fn handle(
        &mut self,
        _message: SourceShutdownMessage,
        ctx: &mut ActorContext<Self>,
    ) -> () {
        if self.streams.is_empty() {
            self.current = None;
            self.retired.clear();
            ctx.stop();
        } else {
            self.frozen = true;
        }
    }
}
