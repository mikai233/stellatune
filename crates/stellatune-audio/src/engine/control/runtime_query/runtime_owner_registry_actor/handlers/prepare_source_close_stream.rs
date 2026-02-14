use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::{RuntimeOwnerRegistryActor, SourceCloseTarget};

pub(crate) struct PrepareSourceCloseStreamMessage {
    pub stream_id: u64,
}

impl Message for PrepareSourceCloseStreamMessage {
    type Response = SourceCloseTarget;
}

#[async_trait::async_trait]
impl Handler<PrepareSourceCloseStreamMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: PrepareSourceCloseStreamMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> SourceCloseTarget {
        let Some(slot) = self.source_stream_slots.get(&message.stream_id).cloned() else {
            return SourceCloseTarget::MissingStream;
        };
        let Some(handle) = self.source_tasks.get(&slot) else {
            return SourceCloseTarget::MissingTask;
        };
        SourceCloseTarget::Ready {
            slot,
            actor_ref: handle.actor_ref.clone(),
        }
    }
}
