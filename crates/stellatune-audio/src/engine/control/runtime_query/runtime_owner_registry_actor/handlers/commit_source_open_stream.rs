use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::runtime_owner_registry_actor::RuntimeOwnerRegistryActor;

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
