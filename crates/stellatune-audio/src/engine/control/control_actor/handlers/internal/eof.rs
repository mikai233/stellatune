use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::super::super::{
    Event, PlayerState, SessionStopMode, event_path_from_engine_token, set_state,
    stop_decode_session,
};
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct EofInternalMessage;

impl Message for EofInternalMessage {
    type Response = ();
}

impl Handler<EofInternalMessage> for ControlActor {
    fn handle(&mut self, _message: EofInternalMessage, _ctx: &mut ActorContext<Self>) {
        self.events.emit(Event::Log {
            message: "end of stream".to_string(),
        });
        if self.state.wants_playback
            && let Some(path) = self.state.current_track.clone()
        {
            self.events.emit(Event::PlaybackEnded {
                path: event_path_from_engine_token(&path),
            });
        }
        stop_decode_session(&mut self.state, &self.track_info, SessionStopMode::KeepSink);
        self.state.seek_position_guard = None;
        self.state.wants_playback = false;
        self.state.play_request_started_at = None;
        self.state.pending_session_start = false;
        set_state(&mut self.state, &self.events, PlayerState::Stopped);
    }
}
