use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::RuntimeOwnerRegistryActor;
use crate::engine::control::RuntimeInstanceSlotKey;

pub(crate) struct CommitSourceOpenStreamMessage {
    pub stream_id: u64,
    pub slot: RuntimeInstanceSlotKey,
}

impl Message for CommitSourceOpenStreamMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<CommitSourceOpenStreamMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: CommitSourceOpenStreamMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        self.source_stream_slots
            .insert(message.stream_id, message.slot);
    }
}
