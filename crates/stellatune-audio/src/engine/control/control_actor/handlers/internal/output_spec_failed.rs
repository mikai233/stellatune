use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tracing::warn;

use super::super::super::super::{Event, PlayerState, set_state};
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct OutputSpecFailedInternalMessage {
    pub(crate) message: String,
    pub(crate) took_ms: u64,
    pub(crate) token: u64,
}

impl Message for OutputSpecFailedInternalMessage {
    type Response = ();
}

impl Handler<OutputSpecFailedInternalMessage> for ControlActor {
    fn handle(&mut self, message: OutputSpecFailedInternalMessage, _ctx: &mut ActorContext<Self>) {
        if message.token != self.state.output_spec_token {
            return;
        }
        self.state.cached_output_spec = None;
        self.state.output_spec_prewarm_inflight = false;
        warn!(
            "output_spec prewarm failed in {}ms: {}",
            message.took_ms, message.message
        );
        if self.state.wants_playback && self.state.session.is_none() {
            self.state.pending_session_start = false;
            self.state.wants_playback = false;
            self.state.play_request_started_at = None;
            self.events.emit(Event::Error {
                message: format!("failed to query output device: {}", message.message),
            });
            set_state(&mut self.state, &self.events, PlayerState::Stopped);
        }
    }
}
