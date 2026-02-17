use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::runtime_query::output_sink_owner_actor::OutputSinkOwnerActor;

pub(crate) struct OutputSinkShutdownMessage;

impl Message for OutputSinkShutdownMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<OutputSinkShutdownMessage> for OutputSinkOwnerActor {
    async fn handle(
        &mut self,
        _message: OutputSinkShutdownMessage,
        ctx: &mut ActorContext<Self>,
    ) -> () {
        self.entry = None;
        ctx.stop();
    }
}
