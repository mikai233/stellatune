use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{
    PlayerState, SessionStopMode, emit_position_event, next_position_session_id, set_state,
    stop_decode_session,
};

pub(crate) struct StopMessage;

impl Message for StopMessage {
    type Response = Result<(), String>;
}

impl Handler<StopMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: StopMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, events, track_info) = (&mut self.state, &self.events, &self.track_info);
        stop_decode_session(state, track_info, SessionStopMode::TearDownSink);
        state.position_ms = 0;
        state.wants_playback = false;
        state.play_request_started_at = None;
        state.pending_session_start = false;
        state.seek_position_guard = None;
        next_position_session_id(state);
        emit_position_event(state, events);
        set_state(state, events, PlayerState::Stopped);
        Ok(())
    }
}
