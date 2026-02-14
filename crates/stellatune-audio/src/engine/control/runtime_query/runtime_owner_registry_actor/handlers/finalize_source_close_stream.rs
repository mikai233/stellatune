use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};
use tracing::warn;

use super::super::RuntimeOwnerRegistryActor;
use crate::engine::control::RuntimeInstanceSlotKey;
use crate::engine::control::runtime_query::OWNER_WORKER_CLEAR_TIMEOUT;
use crate::engine::control::runtime_query::source_owner_actor::handlers::shutdown::SourceShutdownMessage;

pub(crate) struct FinalizeSourceCloseStreamMessage {
    pub slot: RuntimeInstanceSlotKey,
    pub stream_id: u64,
}

impl Message for FinalizeSourceCloseStreamMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<FinalizeSourceCloseStreamMessage> for RuntimeOwnerRegistryActor {
    async fn handle(
        &mut self,
        message: FinalizeSourceCloseStreamMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        self.source_stream_slots.remove(&message.stream_id);
        let mut shutdown_ref: Option<
            stellatune_runtime::tokio_actor::ActorRef<
                crate::engine::control::runtime_query::source_owner_actor::SourceOwnerActor,
            >,
        > = None;
        if let Some(handle) = self.source_tasks.get_mut(&message.slot) {
            handle.active_streams = handle.active_streams.saturating_sub(1);
            if handle.active_streams == 0 && handle.frozen {
                shutdown_ref = Some(handle.actor_ref.clone());
            }
        }
        if shutdown_ref.is_some() {
            self.source_tasks.remove(&message.slot);
        }
        if let Some(actor_ref) = shutdown_ref {
            match actor_ref
                .call(SourceShutdownMessage, OWNER_WORKER_CLEAR_TIMEOUT)
                .await
            {
                Ok(()) => {}
                Err(stellatune_runtime::tokio_actor::CallError::Timeout) => {
                    warn!("source owner task shutdown timeout");
                }
                Err(_) => {}
            }
        }
    }
}
