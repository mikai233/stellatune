use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::OnDecodeLoopEventMessage;
use crate::types::{Event, PlayerState};
use crate::worker::decode_loop::DecodeLoopEvent;

impl Handler<OnDecodeLoopEventMessage> for ControlActor {
    fn handle(&mut self, message: OnDecodeLoopEventMessage, _ctx: &mut ActorContext<Self>) {
        match message.event {
            DecodeLoopEvent::StateChanged(state) => {
                self.update_state(state);
            },
            DecodeLoopEvent::TrackChanged { track_token } => {
                self.snapshot.current_track = Some(track_token.clone());
                self.snapshot.position_ms = 0;
                self.events.emit(Event::TrackChanged { track_token });
                self.update_position(0);
            },
            DecodeLoopEvent::Recovering {
                attempt,
                backoff_ms,
            } => {
                self.events.emit(Event::Recovering {
                    attempt,
                    backoff_ms,
                });
            },
            DecodeLoopEvent::Position { position_ms } => {
                self.update_position(position_ms);
            },
            DecodeLoopEvent::Eof => {
                self.events.emit(Event::Eof);
                self.snapshot.current_track = None;
                self.update_position(0);
                self.update_state(PlayerState::Stopped);
            },
            DecodeLoopEvent::Error(message) => {
                self.emit_error(message);
                self.snapshot.current_track = None;
                self.update_state(PlayerState::Stopped);
            },
        }
    }
}
