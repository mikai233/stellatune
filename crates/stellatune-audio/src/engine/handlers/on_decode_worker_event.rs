use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};

use crate::config::engine::{Event, PlaybackState};
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::OnDecodeWorkerEventMessage;
use crate::workers::decode::DecodeWorkerEvent;

impl Handler<OnDecodeWorkerEventMessage> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: OnDecodeWorkerEventMessage,
    ) -> Result<(), ActorError> {
        match message.event {
            DecodeWorkerEvent::TrackChanged { track_token } => {
                self.current_track = Some(track_token.clone());
                self.position_ms = 0;
                self.events.emit(Event::TrackChanged { track_token });
                self.update_position(0);
            },
            DecodeWorkerEvent::Recovering {
                attempt,
                backoff_ms,
            } => {
                self.transition_state(ctx, PlaybackState::Recovering);
                self.events.emit(Event::Recovering {
                    attempt,
                    backoff_ms,
                });
            },
            DecodeWorkerEvent::Position { position_ms } => {
                if *ctx.behavior() != PlaybackState::Reconfiguring {
                    self.update_position(position_ms);
                }
            },
            DecodeWorkerEvent::AudioStart => {
                self.events.emit(Event::AudioStart);
            },
            DecodeWorkerEvent::AudioEnd => {
                self.events.emit(Event::AudioEnd);
            },
            DecodeWorkerEvent::Eof => {
                self.events.emit(Event::Eof);
                self.current_track = None;
                self.update_position(0);
                self.transition_state(ctx, PlaybackState::Idle);
            },
            DecodeWorkerEvent::Error(error) => {
                self.emit_error(error.to_string());
                self.current_track = None;
                self.transition_state(ctx, PlaybackState::Idle);
            },
        }
        Ok(())
    }
}
