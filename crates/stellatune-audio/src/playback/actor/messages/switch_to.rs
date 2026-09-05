//! Selects an explicit target, reusing the successor when its identity matches.

use super::super::PlaybackActor;
use super::super::preparation::advance_generation;
use super::ControlResult;
use crate::{
    planner::PlaybackRequest,
    playback::{
        control::{SwitchOptions, SwitchTransition},
        event::PlaybackState,
        lifecycle::{reject_pending, set_state, stop_current},
        state::PreparationPurpose,
    },
};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::error::FailureStage;
use stellatune_audio_core::{
    error::PlaybackControlError, playback::PlaybackItem, source::SourceOpenPurpose,
};

/// Selects an explicit target, reusing the successor when its identity matches.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct SwitchTo {
    pub(in crate::playback) item: PlaybackItem,
    pub(in crate::playback) options: SwitchOptions,
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
            self.session.wants_playing = request.options.autoplay;
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
                    FailureStage::Planner,
                    error.to_string(),
                )));
                return Ok(());
            },
        };
        self.session.wants_playing = request.options.autoplay;
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
                PreparationPurpose::Current,
                reply_to,
            );
        }
        Ok(())
    }
}
