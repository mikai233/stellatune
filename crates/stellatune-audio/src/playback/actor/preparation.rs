//! Shared preparation scheduling, cancellation, deadlines, and recovery retries.

use super::{
    PlaybackActor,
    messages::{
        ControlResult, preparation_completed::PreparationCompleted,
        preparation_deadline_elapsed::PreparationDeadlineElapsed,
        recovery_completed::RecoveryCompleted,
    },
};
use crate::playback::{
    event::PlaybackState,
    lifecycle::{fail_current, set_state},
    preparation::prepare_off_turn,
    state::{
        NextTrack, PendingPreparation, PlaybackSession, PreparationPurpose, RecoveryPreparation,
    },
};
use lattice_actor::{context::HandlerContext, reply::ReplyTo};
use std::time::Instant;
use stellatune_audio_core::{
    playback::PlaybackItemId,
    source::{SourceCancellation, SourceOpenPurpose},
};

impl PlaybackActor {
    fn begin_preparation(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        purpose: PreparationPurpose,
        item_id: PlaybackItemId,
    ) -> (u64, u64, SourceCancellation, Instant) {
        self.session.next_preparation_id = self.session.next_preparation_id.wrapping_add(1);
        let id = self.session.next_preparation_id;
        let generation = self.session.generation;
        let timeout = self.config.command_timeouts.preparation;
        let deadline = Instant::now() + timeout;
        let cancellation = SourceCancellation::default();
        let pending = PendingPreparation {
            item_id,
            cancellation: cancellation.clone(),
            id,
            generation,
            purpose,
            deadline,
        };
        if matches!(purpose, PreparationPurpose::Next) {
            self.session.next = NextTrack::Preparing(pending);
        } else {
            self.session.pending_preparation = Some(pending);
        }
        ctx.notify_after(timeout, PreparationDeadlineElapsed { id, generation });
        (id, generation, cancellation, deadline)
    }

    pub(super) fn defer_preparation(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        plan: crate::planner::ExecutablePlaybackPlan,
        source_purpose: SourceOpenPurpose,
        purpose: PreparationPurpose,
        reply_to: ReplyTo<ControlResult>,
    ) {
        let (id, generation, cancellation, deadline) =
            self.begin_preparation(ctx, purpose, plan.item.id);
        if ctx
            .defer_reply(
                reply_to,
                prepare_off_turn(
                    plan,
                    id,
                    generation,
                    source_purpose,
                    purpose,
                    cancellation,
                    deadline,
                ),
                |prepared, reply_to| PreparationCompleted { prepared, reply_to },
            )
            .is_err()
        {
            self.cancel_preparation(purpose);
            let mut state = *ctx.behavior();
            match purpose {
                PreparationPurpose::Current { .. } => {
                    set_state(&mut state, PlaybackState::Failed, &self.event_tx);
                },
                PreparationPurpose::Next => self.session.next.clear(),
                PreparationPurpose::Recovery { .. } => {},
            }
            self.transition(ctx, state);
        }
    }

    pub(super) fn launch_pending_recovery(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        state: &mut PlaybackState,
    ) {
        let Some(recovery) = self.session.pending_recovery.take() else {
            return;
        };
        let timeout = self.config.command_timeouts.preparation;
        let deadline = Instant::now() + timeout;
        self.session.pending_preparation = Some(PendingPreparation {
            item_id: recovery.plan.item.id,
            cancellation: recovery.cancellation.clone(),
            id: recovery.id,
            generation: recovery.generation,
            purpose: recovery.purpose,
            deadline,
        });
        ctx.notify_after(
            timeout,
            PreparationDeadlineElapsed {
                id: recovery.id,
                generation: recovery.generation,
            },
        );
        let task = prepare_off_turn(
            recovery.plan,
            recovery.id,
            recovery.generation,
            SourceOpenPurpose::Recovery,
            recovery.purpose,
            recovery.cancellation,
            deadline,
        );
        if ctx
            .pipe_to_self(task, |prepared| RecoveryCompleted { prepared })
            .is_err()
        {
            self.session.pending_preparation = None;
            fail_current(
                &mut self.session,
                state,
                &self.event_tx,
                "runtime",
                "playback preparation capacity is exhausted".to_owned(),
            );
        }
    }

    pub(super) fn audit_preparation_deadline(&mut self, ctx: &mut HandlerContext<'_, Self>) {
        let expired: Vec<_> = self
            .session
            .pending_preparation
            .iter()
            .chain(self.session.next.pending())
            .filter(|pending| Instant::now() >= pending.deadline)
            .map(|pending| PreparationDeadlineElapsed {
                id: pending.id,
                generation: pending.generation,
            })
            .collect();
        for message in expired {
            self.handle_preparation_timeout(ctx, message);
        }
    }

    pub(super) fn cancel_preparation(&mut self, purpose: PreparationPurpose) {
        if matches!(purpose, PreparationPurpose::Next) {
            self.session.next.clear();
            self.session.advance_options = None;
            self.session.force_transition = false;
        } else if let Some(pending) = self.session.pending_preparation.take() {
            pending.cancellation.cancel();
        }
    }

    pub(super) fn schedule_recovery_retry(&mut self, purpose: PreparationPurpose) {
        let PreparationPurpose::Recovery {
            item_id,
            checkpoint,
            resume_state,
            attempt,
        } = purpose
        else {
            return;
        };
        let Some(current) = self.session.current.as_ref() else {
            return;
        };
        self.session.next_preparation_id = self.session.next_preparation_id.wrapping_add(1);
        self.session.pending_recovery = Some(RecoveryPreparation {
            plan: current.recovery_plan.clone(),
            id: self.session.next_preparation_id,
            generation: self.session.generation,
            purpose: PreparationPurpose::Recovery {
                item_id,
                checkpoint,
                resume_state,
                attempt: attempt + 1,
            },
            cancellation: SourceCancellation::default(),
        });
    }
}

/// Invalidates outstanding preparation and advances the session generation.
pub(super) fn advance_generation(session: &mut PlaybackSession) {
    if let Some(pending) = session.pending_preparation.take() {
        pending.cancellation.cancel();
    }
    if let Some(pending) = session.pending_recovery.take() {
        pending.cancellation.cancel();
    }
    session.next.clear();
    session.advance_options = None;
    session.generation = session.generation.wrapping_add(1);
}
