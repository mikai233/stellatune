//! Advances one bounded playback data-plane turn.

use super::super::PlaybackActor;
use crate::playback::{
    event::PlaybackState,
    lifecycle::{advance_pending_seek, publish_control_failure},
    pump::pump_once,
};
use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};

/// Advances one bounded playback data-plane turn.
#[derive(lattice_actor::Message)]
pub(in crate::playback) struct PumpAudio;

impl Handler<PumpAudio> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: PumpAudio,
    ) -> Result<(), ActorError> {
        self.audit_preparation_deadline(ctx);
        let mut state = *ctx.behavior();
        advance_pending_seek(&self.event_tx, &mut self.session, &mut state);
        if matches!(state, PlaybackState::Playing | PlaybackState::Buffering)
            && self.session.pending_seek.is_none()
        {
            pump_once(&self.config, &self.event_tx, &mut self.session, &mut state);
        }
        self.launch_pending_recovery(ctx, &mut state);
        if let Err(error) = self.apply_advance(&mut state) {
            publish_control_failure(&error, &self.event_tx);
        }
        self.transition(ctx, state);
        Ok(())
    }
}
