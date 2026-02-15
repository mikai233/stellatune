use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::runtime_owner_registry_actor::RuntimeOwnerRegistryActor;

pub(crate) struct RollbackSourceOpenStreamMessage {
    pub slot: RuntimeInstanceSlotKey,
}

impl Message for RollbackSourceOpenStreamMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<RollbackSourceOpenStreamMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: RollbackSourceOpenStreamMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Some(handle) = self.source_tasks.get_mut(&message.slot) {
            handle.active_streams = handle.active_streams.saturating_sub(1);
        }
    }
}
