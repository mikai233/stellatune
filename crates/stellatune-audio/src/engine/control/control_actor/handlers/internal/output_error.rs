use std::time::Instant;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tracing::error;

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{
    Event, PlayerState, SessionStopMode, drop_output_pipeline, ensure_output_spec_prewarm,
    set_state, stop_all_audio, stop_decode_session,
};

pub(crate) struct OutputErrorInternalMessage {
    pub(crate) message: String,
}

impl Message for OutputErrorInternalMessage {
    type Response = ();
}

impl Handler<OutputErrorInternalMessage> for ControlActor {
    fn handle(&mut self, message: OutputErrorInternalMessage, _ctx: &mut ActorContext<Self>) {
        if self.state.session.is_none() {
            error!(
                "output stream error (no active session): {}",
                message.message
            );
            self.events.emit(Event::Log {
                message: format!(
                    "output stream error (no active session): {}",
                    message.message
                ),
            });
            return;
        }

        error!("output stream error: {}", message.message);
        self.events.emit(Event::Error {
            message: format!("output stream error: {}", message.message),
        });

        if self.state.current_track.is_none() {
            stop_all_audio(&mut self.state, &self.track_info);
            self.state.wants_playback = false;
            set_state(&mut self.state, &self.events, PlayerState::Stopped);
            return;
        }

        let prev_state = self.state.player_state;
        stop_decode_session(
            &mut self.state,
            &self.track_info,
            SessionStopMode::TearDownSink,
        );
        drop_output_pipeline(&mut self.state);
        self.state.seek_position_guard = None;

        self.state.cached_output_spec = None;
        ensure_output_spec_prewarm(&mut self.state, &self.internal_tx);
        self.state.pending_session_start =
            prev_state == PlayerState::Playing || prev_state == PlayerState::Buffering;

        match prev_state {
            PlayerState::Playing | PlayerState::Buffering => {
                self.state.wants_playback = true;
                self.state.play_request_started_at = Some(Instant::now());
                set_state(&mut self.state, &self.events, PlayerState::Buffering);
            },
            PlayerState::Paused => {
                self.state.wants_playback = false;
                self.state.play_request_started_at = None;
                set_state(&mut self.state, &self.events, PlayerState::Paused);
            },
            PlayerState::Stopped => {
                self.state.wants_playback = false;
                self.state.play_request_started_at = None;
                set_state(&mut self.state, &self.events, PlayerState::Stopped);
            },
        }

        self.events.emit(Event::Log {
            message: "output error: scheduled session restart".to_string(),
        });
    }
}
