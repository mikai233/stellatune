use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::OnDecodeWorkerEventMessage;
use crate::types::{Event, PlayerState};
use crate::workers::decode_worker::DecodeWorkerEvent;

impl Handler<OnDecodeWorkerEventMessage> for ControlActor {
    fn handle(&mut self, message: OnDecodeWorkerEventMessage, _ctx: &mut ActorContext<Self>) {
        match message.event {
            DecodeWorkerEvent::StateChanged(state) => {
                self.update_state(state);
            },
            DecodeWorkerEvent::TrackChanged { track_token } => {
                self.snapshot.current_track = Some(track_token.clone());
                self.snapshot.position_ms = 0;
                self.events.emit(Event::TrackChanged { track_token });
                self.update_position(0);
            },
            DecodeWorkerEvent::Recovering {
                attempt,
                backoff_ms,
            } => {
                self.events.emit(Event::Recovering {
                    attempt,
                    backoff_ms,
                });
            },
            DecodeWorkerEvent::Position { position_ms } => {
                self.update_position(position_ms);
            },
            DecodeWorkerEvent::Eof => {
                self.events.emit(Event::Eof);
                self.snapshot.current_track = None;
                self.update_position(0);
                self.update_state(PlayerState::Stopped);
            },
            DecodeWorkerEvent::Error(message) => {
                self.emit_error(message);
                self.snapshot.current_track = None;
                self.update_state(PlayerState::Stopped);
            },
        }
    }
}
