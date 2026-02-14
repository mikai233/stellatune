use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::SourceOwnerActor;

pub(crate) struct SourceFreezeMessage;

impl Message for SourceFreezeMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<SourceFreezeMessage> for SourceOwnerActor {
    async fn handle(&mut self, _message: SourceFreezeMessage, _ctx: &mut ActorContext<Self>) -> () {
        self.frozen = true;
    }
}
