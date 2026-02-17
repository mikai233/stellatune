use stellatune_runtime::tokio_actor::{ActorContext, ActorRef, Handler, Message, spawn_actor};

use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::runtime_owner_registry_actor::{
    RuntimeOwnerRegistryActor, SourceOwnerTaskHandle,
};
use crate::engine::control::runtime_query::source_owner_actor::SourceOwnerActor;

pub(crate) struct EnsureSourceOwnerTaskMessage {
    pub slot: RuntimeInstanceSlotKey,
}

impl Message for EnsureSourceOwnerTaskMessage {
    type Response = ActorRef<SourceOwnerActor>;
}

#[async_trait::async_trait]
impl Handler<EnsureSourceOwnerTaskMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: EnsureSourceOwnerTaskMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> ActorRef<SourceOwnerActor> {
        if let Some(handle) = self.source_tasks.get(&message.slot)
            && !handle.actor_ref.is_closed()
        {
            return handle.actor_ref.clone();
        }

        let plugin_id = message.slot.plugin_id.clone();
        let type_id = message.slot.type_id.clone();
        let active_streams = self
            .source_tasks
            .get(&message.slot)
            .map(|h| h.active_streams)
            .unwrap_or(0);
        let (actor_ref, _join) = spawn_actor(SourceOwnerActor::new(plugin_id, type_id));
        self.source_tasks.insert(
            message.slot,
            SourceOwnerTaskHandle {
                actor_ref: actor_ref.clone(),
                active_streams,
                frozen: false,
            },
        );
        actor_ref
    }
}
