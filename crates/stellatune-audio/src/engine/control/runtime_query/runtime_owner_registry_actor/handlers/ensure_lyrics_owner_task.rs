use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::{LyricsOwnerTaskHandle, RuntimeOwnerRegistryActor};
use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::lyrics_owner_actor::LyricsOwnerActor;

pub(crate) struct EnsureLyricsOwnerTaskMessage {
    pub slot: RuntimeInstanceSlotKey,
}

impl Message for EnsureLyricsOwnerTaskMessage {
    type Response = stellatune_runtime::tokio_actor::ActorRef<LyricsOwnerActor>;
}

#[async_trait::async_trait]
impl Handler<EnsureLyricsOwnerTaskMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: EnsureLyricsOwnerTaskMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> stellatune_runtime::tokio_actor::ActorRef<LyricsOwnerActor> {
        if let Some(handle) = self.lyrics_tasks.get(&message.slot)
            && !handle.actor_ref.is_closed()
        {
            return handle.actor_ref.clone();
        }
        let plugin_id = message.slot.plugin_id.clone();
        let type_id = message.slot.type_id.clone();
        let (actor_ref, _join) =
            stellatune_runtime::tokio_actor::spawn_actor(LyricsOwnerActor::new(plugin_id, type_id));
        self.lyrics_tasks.insert(
            message.slot,
            LyricsOwnerTaskHandle {
                actor_ref: actor_ref.clone(),
                frozen: false,
            },
        );
        actor_ref
    }
}
