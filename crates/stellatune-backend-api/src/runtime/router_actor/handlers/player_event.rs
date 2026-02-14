use std::sync::atomic::Ordering;

use stellatune_core::{Event, HostEventTopic, HostPlayerEventEnvelope};
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::RuntimeRouterActor;
use crate::runtime::bus::{
    ControlFinishedArgs, broadcast_host_event_json, drain_finished_by_player_event,
    emit_control_finished,
};

pub(crate) struct PlayerEventMessage {
    pub(crate) generation: u64,
    pub(crate) event: Event,
}

impl Message for PlayerEventMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<PlayerEventMessage> for RuntimeRouterActor {
    async fn handle(&mut self, message: PlayerEventMessage, _ctx: &mut ActorContext<Self>) -> () {
        let current = self.router.player_event_generation.load(Ordering::Relaxed);
        if message.generation != current {
            return;
        }

        let done = drain_finished_by_player_event(&mut self.pending_finishes, &message.event);

        if !matches!(message.event, Event::Position { .. } | Event::Log { .. }) {
            let payload = HostPlayerEventEnvelope {
                topic: HostEventTopic::PlayerEvent,
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
