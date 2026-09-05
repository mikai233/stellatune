//! Advances one bounded playback data-plane turn.

use super::super::PlaybackActor;
use crate::playback::{
    event::PlaybackState,
    lifecycle::{advance_pending_seek, publish_control_failure},
    pump::pump_once,
};
use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};
use stellatune_audio_core::error::FailureCode;
use stellatune_audio_core::error::FailureStage;

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
        if self.session.pending_recovery.is_none()
            && !self
                .session
                .pending_preparation
                .as_ref()
                .is_some_and(|pending| {
                    matches!(
                        pending.purpose,
                        crate::playback::state::PreparationPurpose::Recovery { .. }
                    )
                })
            && let Some(error) = self
                .session
                .current
                .as_ref()
                .and_then(|track| track.output.try_failure())
        {
            if self
                .session
                .current
                .as_ref()
                .is_some_and(|track| track.output.is_initialized())
                && matches!(&error, stellatune_audio_core::error::PlaybackControlError::Failed(failure)
                if failure.stage == FailureStage::Sink
                && matches!(failure.code, FailureCode::Io | FailureCode::StageFailed))
            {
                crate::playback::pump::begin_recovery(
                    &self.config,
                    &self.event_tx,
                    &mut self.session,
                    &mut state,
                    error,
                );
            } else {
                crate::playback::lifecycle::fail_current_error(
                    &mut self.session,
                    &mut state,
                    &self.event_tx,
                    error,
                );
            }
        }
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
