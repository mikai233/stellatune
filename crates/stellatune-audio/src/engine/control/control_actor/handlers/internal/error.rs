use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{Event, PlayerState, SessionStopMode, set_state, stop_decode_session};

pub(crate) struct ErrorInternalMessage {
    pub(crate) message: String,
}

impl Message for ErrorInternalMessage {
    type Response = ();
}

impl Handler<ErrorInternalMessage> for ControlActor {
    fn handle(&mut self, message: ErrorInternalMessage, _ctx: &mut ActorContext<Self>) {
        self.events.emit(Event::Error {
            message: message.message,
        });
        stop_decode_session(
            &mut self.state,
            &self.track_info,
            SessionStopMode::TearDownSink,
        );
        self.state.seek_position_guard = None;
        self.state.wants_playback = false;
        self.state.play_request_started_at = None;
        self.state.pending_session_start = false;
        set_state(&mut self.state, &self.events, PlayerState::Stopped);
    }
}
