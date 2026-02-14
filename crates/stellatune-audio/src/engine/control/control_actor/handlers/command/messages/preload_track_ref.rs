use stellatune_core::TrackRef;
use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::{debug_metrics, enqueue_preload_task, track_ref_to_engine_token};
use super::emit_and_err;
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct PreloadTrackRefMessage {
    pub(crate) track: TrackRef,
    pub(crate) position_ms: u64,
}

impl Message for PreloadTrackRefMessage {
    type Response = Result<(), String>;
}

impl Handler<PreloadTrackRefMessage> for ControlActor {
    fn handle(
        &mut self,
        message: PreloadTrackRefMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, events) = (&mut self.state, &self.events);
        let Some(path) = track_ref_to_engine_token(&message.track) else {
            return emit_and_err(events, "track locator is empty");
        };
        if state.requested_preload_path.as_deref() == Some(path.as_str())
            && state.requested_preload_position_ms == message.position_ms
        {
            return Ok(());
        }
        state.requested_preload_path = Some(path.clone());
        state.requested_preload_position_ms = message.position_ms;
        state.preload_token = state.preload_token.wrapping_add(1);
        debug_metrics::note_preload_request();
        enqueue_preload_task(state, path, message.position_ms, state.preload_token);
        Ok(())
    }
}
