//! Explicit target selection, successor preparation, and atomic advancement.
//!
//! A successor retains its queue-item identity throughout preparation. Advancing
//! claims that existing work; only replacing the slot cancels its source token.

use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::{
    error::PlaybackControlError,
    playback::{PlaybackItem, PlaybackItemId},
    source::SourceOpenPurpose,
};

use super::{
    actor::{PlaybackActor, advance_generation},
    control::{AdvanceOutcome, SwitchOptions, SwitchTransition},
    event::{PlaybackEvent, PlaybackState},
    lifecycle::{reject_pending, set_state, stop_current},
    pump::activate,
    state::PreparationPurpose,
    transition::configure_forced_transition,
};
use crate::planner::PlaybackRequest;

type ControlResult = Result<(), PlaybackControlError>;
type AdvanceResult = Result<AdvanceOutcome, PlaybackControlError>;

/// Selects an explicit target, reusing the successor when its identity matches.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct SwitchTo {
    pub(super) item: PlaybackItem,
    pub(super) options: SwitchOptions,
}

/// Replaces or clears the successor without replacing the active session.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct SetNext {
    pub(super) item: Option<PlaybackItem>,
}

/// Claims a specific successor atomically, including its in-flight preparation.
#[derive(lattice_actor::Request)]
#[request(response = AdvanceResult)]
pub(super) struct AdvanceToNext {
    pub(super) expected_item_id: PlaybackItemId,
    pub(super) options: SwitchOptions,
}

impl PlaybackActor {
    fn apply_overlap_intent(
        &self,
        state: &mut PlaybackState,
        options: SwitchOptions,
    ) -> ControlResult {
        let current = self
            .session
            .current
            .as_ref()
            .ok_or(PlaybackControlError::InvalidState)?;
        if options.autoplay {
            current.output.resume()?;
            set_state(state, PlaybackState::Playing, &self.event_tx);
        } else {
            current.output.pause()?;
            set_state(state, PlaybackState::Paused, &self.event_tx);
        }
        Ok(())
    }

    fn matches_successor(&self, id: PlaybackItemId) -> bool {
        self.session.next.item_id() == Some(id)
            || self
                .session
                .crossfade
                .as_ref()
                .is_some_and(|fade| fade.next.item_id == id)
    }

    /// Applies a claimed successor once ready; an already-started overlap is retained.
    pub(super) fn apply_advance(&mut self, state: &mut PlaybackState) -> ControlResult {
        let Some(options) = self.session.advance_options else {
            return Ok(());
        };
        if self.session.crossfade.is_some() {
            self.apply_overlap_intent(state, options)?;
            return Ok(());
        }
        if self.session.next.as_mut().is_none() {
            return Ok(());
        }
        self.session.advance_options = None;
        reject_pending(&mut self.session);
        if options.transition == SwitchTransition::UseConfiguredPolicy
            && options.autoplay
            && *state != PlaybackState::Recovering
            && self.session.current.is_some()
        {
            self.session.force_transition = true;
            configure_forced_transition(&mut self.session);
            self.session
                .current
                .as_ref()
                .expect("current checked")
                .output
                .resume()?;
            set_state(state, PlaybackState::Playing, &self.event_tx);
            return Ok(());
        }
        let next = self.session.next.take().expect("ready checked");
        if let Some(pending) = self.session.pending_preparation.take() {
            pending.cancellation.cancel();
        }
        if let Some(pending) = self.session.pending_recovery.take() {
            pending.cancellation.cancel();
        }
        stop_current(&mut self.session);
        self.session.force_transition = false;
        let mut current = match activate(next, &self.config, self.session.output_gain) {
            Ok(current) => current,
            Err(error) => {
                set_state(state, PlaybackState::Failed, &self.event_tx);
                return Err(error);
            },
        };
        current.fade_in_frames = current.seek_fade_frames;
        let item_id = current.item_id;
        if options.autoplay {
            current.output.resume()?;
        } else {
            current.output.pause()?;
        }
        self.session.current = Some(current);
        set_state(
            state,
            if options.autoplay {
                PlaybackState::Playing
            } else {
                PlaybackState::Ready
            },
            &self.event_tx,
        );
        let _ = self.event_tx.send(PlaybackEvent::TrackChanged { item_id });
        Ok(())
    }
}

impl Responder<AdvanceToNext> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        request: AdvanceToNext,
        reply_to: ReplyTo<AdvanceResult>,
    ) -> Result<(), ActorError> {
        if !self.matches_successor(request.expected_item_id) {
            let already_current = self.session.crossfade.is_none()
                && self
                    .session
                    .current
                    .as_ref()
                    .is_some_and(|current| current.item_id == request.expected_item_id);
            let _ = reply_to.send(Ok(if already_current {
                AdvanceOutcome::AlreadyCurrent
            } else {
                AdvanceOutcome::Unavailable
            }));
            return Ok(());
        }
        if self
            .session
            .crossfade
            .as_ref()
            .is_some_and(|fade| fade.next.item_id == request.expected_item_id)
        {
            let mut state = *ctx.behavior();
            let result = self
                .apply_overlap_intent(&mut state, request.options)
                .map(|()| AdvanceOutcome::Accepted);
            self.transition(ctx, state);
            let _ = reply_to.send(result);
            return Ok(());
        }
        self.session.advance_options = Some(request.options);
        let mut state = *ctx.behavior();
        let result = self
            .apply_advance(&mut state)
            .map(|()| AdvanceOutcome::Accepted);
        self.transition(ctx, state);
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<SwitchTo> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        request: SwitchTo,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        if self
            .session
            .crossfade
            .as_ref()
            .is_some_and(|fade| fade.next.item_id == request.item.id)
        {
            let mut state = *ctx.behavior();
            let result = self.apply_overlap_intent(&mut state, request.options);
            self.transition(ctx, state);
            let _ = reply_to.send(result);
            return Ok(());
        }
        if self.matches_successor(request.item.id) {
            self.session.advance_options = Some(request.options);
            let mut state = *ctx.behavior();
            let result = self.apply_advance(&mut state);
            self.transition(ctx, state);
            let _ = reply_to.send(result);
            return Ok(());
        }
        // Validate the replacement before invalidating useful existing work.
        let plan = match self.planner.plan(
            PlaybackRequest {
                item: request.item,
                policies: self.session.policies,
            },
            &self.config.registry,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let _ = reply_to.send(Err(PlaybackControlError::failed(
                    "planner",
                    error.to_string(),
                )));
                return Ok(());
            },
        };
        advance_generation(&mut self.session);
        reject_pending(&mut self.session);
        if request.options.transition == SwitchTransition::ImmediateWithDeClick {
            self.session.crossfade = None;
        }
        self.session.force_transition = false;
        if let Some(current) = self.session.current.as_mut() {
            current.forced_end_frame = None;
        }
        let mut state = *ctx.behavior();
        if self.session.current.is_some()
            && request.options.transition == SwitchTransition::UseConfiguredPolicy
        {
            self.session.advance_options = Some(request.options);
            self.defer_preparation(
                ctx,
                plan,
                SourceOpenPurpose::Prewarm,
                PreparationPurpose::Next,
                reply_to,
            );
        } else {
            stop_current(&mut self.session);
            set_state(&mut state, PlaybackState::Preparing, &self.event_tx);
            self.transition(ctx, state);
            self.defer_preparation(
                ctx,
                plan,
                SourceOpenPurpose::Initial,
                PreparationPurpose::Current {
                    autoplay: request.options.autoplay,
                },
                reply_to,
            );
        }
        Ok(())
    }
}

impl Responder<SetNext> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        request: SetNext,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let Some(item) = request.item else {
            self.session.next.clear();
            self.session.advance_options = None;
            self.session.force_transition = false;
            if let Some(current) = self.session.current.as_mut() {
                current.forced_end_frame = None;
            }
            let _ = reply_to.send(Ok(()));
            return Ok(());
        };
        if self.session.current.is_none() {
            let _ = reply_to.send(Err(PlaybackControlError::InvalidState));
            return Ok(());
        }
        if self.matches_successor(item.id) {
            let _ = reply_to.send(Ok(()));
            return Ok(());
        }
        let plan = match self.planner.plan(
            PlaybackRequest {
                item,
                policies: self.session.policies,
            },
            &self.config.registry,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                let _ = reply_to.send(Err(PlaybackControlError::failed(
                    "planner",
                    error.to_string(),
                )));
                return Ok(());
            },
        };
        self.session.next.clear();
        self.session.advance_options = None;
        self.session.force_transition = false;
        if let Some(current) = self.session.current.as_mut() {
            current.forced_end_frame = None;
        }
        self.defer_preparation(
            ctx,
            plan,
            SourceOpenPurpose::Prewarm,
            PreparationPurpose::Next,
            reply_to,
        );
        Ok(())
    }
}
