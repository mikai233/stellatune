use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::{RuntimeOwnerRegistryActor, SourceOwnerTaskHandle};
use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::source_owner_actor::SourceOwnerActor;

pub(crate) struct PrepareSourceOpenStreamMessage {
    pub slot: RuntimeInstanceSlotKey,
}

impl Message for PrepareSourceOpenStreamMessage {
    type Response = (
        stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>,
        u64,
    );
}

#[async_trait::async_trait]
impl Handler<PrepareSourceOpenStreamMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: PrepareSourceOpenStreamMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> (
        stellatune_runtime::tokio_actor::ActorRef<SourceOwnerActor>,
        u64,
    ) {
        let actor_ref = if let Some(handle) = self.source_tasks.get(&message.slot)
            && !handle.actor_ref.is_closed()
        {
            handle.actor_ref.clone()
        } else {
            let plugin_id = message.slot.plugin_id.clone();
            let type_id = message.slot.type_id.clone();
            let active_streams = self
                .source_tasks
                .get(&message.slot)
                .map(|h| h.active_streams)
                .unwrap_or(0);
            let (actor_ref, _join) = stellatune_runtime::tokio_actor::spawn_actor(
                SourceOwnerActor::new(plugin_id, type_id),
            );
            self.source_tasks.insert(
                message.slot.clone(),
                SourceOwnerTaskHandle {
                    actor_ref: actor_ref.clone(),
                    active_streams,
                    frozen: false,
                },
            );
            actor_ref
        };

        if let Some(handle) = self.source_tasks.get_mut(&message.slot) {
            handle.active_streams = handle.active_streams.saturating_add(1);
        }

        let mut stream_id = self.next_source_stream_id;
        if stream_id == 0 {
            stream_id = 1;
        }
        self.next_source_stream_id = stream_id.wrapping_add(1);

        (actor_ref, stream_id)
    }
}
