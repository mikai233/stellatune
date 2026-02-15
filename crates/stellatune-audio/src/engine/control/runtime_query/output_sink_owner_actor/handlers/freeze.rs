use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::runtime_query::output_sink_owner_actor::OutputSinkOwnerActor;

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
