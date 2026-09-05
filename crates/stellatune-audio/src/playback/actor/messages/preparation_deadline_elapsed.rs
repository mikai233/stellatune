//! Cancels preparation that is still current when its deadline expires.

use super::super::PlaybackActor;
use crate::playback::{
    event::PlaybackState,
    lifecycle::{fail_current, set_state},
    pump::promote_or_end,
    state::{DrainPhase, PreparationPurpose},
};
use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};
use stellatune_audio_core::error::FailureStage;

/// Cancels preparation that is still current when its deadline expires.
#[derive(lattice_actor::Message)]
pub(in crate::playback) struct PreparationDeadlineElapsed {
    pub(in crate::playback::actor) id: u64,
    pub(in crate::playback::actor) generation: u64,
}

impl Handler<PreparationDeadlineElapsed> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: PreparationDeadlineElapsed,
    ) -> Result<(), ActorError> {
        self.handle_preparation_timeout(ctx, message);
        Ok(())
    }
}

impl PlaybackActor {
    pub(in crate::playback::actor) fn handle_preparation_timeout(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: PreparationDeadlineElapsed,
    ) {
        let Some(pending) = self
            .session
            .pending_preparation
            .iter()
            .chain(self.session.next.pending())
            .find(|pending| pending.id == message.id && pending.generation == message.generation)
        else {
            return;
        };
        let purpose = pending.purpose;
        self.cancel_preparation(purpose);
        let mut state = *ctx.behavior();
        match purpose {
            PreparationPurpose::Current => {
                set_state(&mut state, PlaybackState::Failed, &self.event_tx);
            },
            PreparationPurpose::Next => {
                self.session.next.clear();
                if state == PlaybackState::Buffering
                    && self
                        .session
                        .current
                        .as_ref()
                        .is_some_and(|current| current.drain_phase == DrainPhase::Complete)
                {
                    promote_or_end(&mut self.session, &mut state, &self.config, &self.event_tx);
                }
            },
            PreparationPurpose::Recovery { .. } => {
                let PreparationPurpose::Recovery {
                    item_id, attempt, ..
                } = purpose
                else {
                    unreachable!()
                };
                let retry_limit = self.session.policies.max_recovery_attempts.max(1);
                if attempt < retry_limit
                    && self
                        .session
                        .current
                        .as_ref()
                        .is_some_and(|current| current.item_id == item_id)
                {
                    self.schedule_recovery_retry(purpose);
                    self.launch_pending_recovery(ctx, &mut state);
                } else {
                    fail_current(
                        &mut self.session,
                        &mut state,
                        &self.event_tx,
                        FailureStage::Runtime,
                        "recovery preparation timed out".to_owned(),
                    );
                }
            },
        }
        self.transition(ctx, state);
    }
}
