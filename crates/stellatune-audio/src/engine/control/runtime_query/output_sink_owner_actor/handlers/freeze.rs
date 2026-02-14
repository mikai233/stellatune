use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::OutputSinkOwnerActor;

pub(crate) struct OutputSinkFreezeMessage;

impl Message for OutputSinkFreezeMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<OutputSinkFreezeMessage> for OutputSinkOwnerActor {
    async fn handle(
        &mut self,
        _message: OutputSinkFreezeMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        self.frozen = true;
    }
}
