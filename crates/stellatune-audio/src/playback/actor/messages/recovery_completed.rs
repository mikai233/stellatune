//! Returns recovery preparation that has no external request reply.

use super::super::PlaybackActor;
use crate::playback::{
    event::PlaybackState,
    lifecycle::{fail_current, set_state},
    pump::activate,
    state::{PreparationPurpose, PreparationResult},
};
use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};
use stellatune_audio_core::error::FailureStage;

/// Returns recovery preparation that has no external request reply.
#[derive(lattice_actor::Message)]
pub(in crate::playback) struct RecoveryCompleted {
    pub(in crate::playback::actor) prepared: PreparationResult,
}

impl Handler<RecoveryCompleted> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: RecoveryCompleted,
    ) -> Result<(), ActorError> {
        let prepared = message.prepared;
        let matches = self
            .session
            .pending_preparation
            .as_ref()
            .is_some_and(|pending| {
                pending.id == prepared.id && pending.generation == prepared.generation
            });
        if !matches || prepared.generation != self.session.generation {
            return Ok(());
        }
        self.session.pending_preparation = None;
        let purpose = prepared.purpose;
        let PreparationPurpose::Recovery {
            item_id,
            checkpoint: _,
            attempt,
        } = purpose
        else {
            return Ok(());
        };
        let mut state = *ctx.behavior();
        match prepared.result.and_then(|prepared| {
            activate(
                prepared,
                &self.config,
                self.session.output_gain,
                &self.session.output_workers,
            )
        }) {
            Ok(mut recovered) if recovered.item_id == item_id => {
                if let Some(mut failed) = self.session.current.take() {
                    failed.output.shutdown();
                }
                recovered.fade_in_start_frame = recovered.position_base_frame;
                recovered.fade_in_frames = recovered.seek_fade_frames;
                if self.session.wants_playing {
                    let _ = recovered.output.resume();
                } else {
                    let _ = recovered.output.pause();
                }
                self.session.current = Some(recovered);
                set_state(
                    &mut state,
                    if self.session.wants_playing {
                        PlaybackState::Playing
                    } else {
                        PlaybackState::Paused
                    },
                    &self.event_tx,
                );
            },
            Ok(_) => fail_current(
                &mut self.session,
                &mut state,
                &self.event_tx,
                FailureStage::Runtime,
                "recovery completed for the wrong playback item".to_owned(),
            ),
            Err(error) => {
                let retry_limit = self.session.policies.max_recovery_attempts.max(1);
                if attempt < retry_limit
                    && self
                        .session
                        .current
                        .as_ref()
                        .is_some_and(|current| current.item_id == item_id)
                {
                    self.schedule_recovery_retry(purpose);
                } else {
                    crate::playback::lifecycle::fail_current_error(
                        &mut self.session,
                        &mut state,
                        &self.event_tx,
                        error,
                    );
                }
            },
        }
        self.launch_pending_recovery(ctx, &mut state);
        self.transition(ctx, state);
        Ok(())
    }
}
