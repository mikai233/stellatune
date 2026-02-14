use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::{OutputSinkOwnerTaskHandle, RuntimeOwnerRegistryActor};
use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::output_sink_owner_actor::OutputSinkOwnerActor;

pub(crate) struct EnsureOutputSinkOwnerTaskMessage {
    pub slot: RuntimeInstanceSlotKey,
}

impl Message for EnsureOutputSinkOwnerTaskMessage {
    type Response = stellatune_runtime::tokio_actor::ActorRef<OutputSinkOwnerActor>;
}

#[async_trait::async_trait]
impl Handler<EnsureOutputSinkOwnerTaskMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: EnsureOutputSinkOwnerTaskMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> stellatune_runtime::tokio_actor::ActorRef<OutputSinkOwnerActor> {
        if let Some(handle) = self.output_sink_tasks.get(&message.slot)
            && !handle.actor_ref.is_closed()
        {
            return handle.actor_ref.clone();
        }
        let plugin_id = message.slot.plugin_id.clone();
        let type_id = message.slot.type_id.clone();
        let (actor_ref, _join) = stellatune_runtime::tokio_actor::spawn_actor(
            OutputSinkOwnerActor::new(plugin_id, type_id),
        );
        self.output_sink_tasks.insert(
            message.slot,
            OutputSinkOwnerTaskHandle {
                actor_ref: actor_ref.clone(),
                frozen: false,
            },
        );
        actor_ref
    }
}
