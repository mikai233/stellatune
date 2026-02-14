use std::sync::atomic::Ordering;

use stellatune_core::{HostEventTopic, HostLibraryEventEnvelope, LibraryEvent};
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::RuntimeRouterActor;
use crate::runtime::bus::{
    ControlFinishedArgs, broadcast_host_event_json, drain_finished_by_library_event,
    emit_control_finished,
};

pub(crate) struct LibraryEventMessage {
    pub(crate) generation: u64,
    pub(crate) event: LibraryEvent,
}

impl Message for LibraryEventMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<LibraryEventMessage> for RuntimeRouterActor {
    async fn handle(&mut self, message: LibraryEventMessage, _ctx: &mut ActorContext<Self>) -> () {
        let current = self.router.library_event_generation.load(Ordering::Relaxed);
        if message.generation != current {
            return;
        }

        let done = drain_finished_by_library_event(&mut self.pending_finishes, &message.event);

        if !matches!(message.event, LibraryEvent::Log { .. }) {
            let payload = HostLibraryEventEnvelope {
                topic: HostEventTopic::LibraryEvent,
                event: message.event,
            };
            if let Ok(payload_json) = serde_json::to_string(&payload) {
                broadcast_host_event_json(payload_json).await;
            }
        }

        for done_item in done {
            emit_control_finished(
                self.router.runtime_hub.as_ref(),
                ControlFinishedArgs {
                    plugin_id: &done_item.plugin_id,
                    request_id: done_item.request_id,
                    scope: done_item.scope,
                    command: done_item.command,
                    error: None,
                },
            )
            .await;
        }
    }
}
