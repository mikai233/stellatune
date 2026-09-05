//! Replaces or clears the successor without replacing the active session.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::{planner::PlaybackRequest, playback::state::PreparationPurpose};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::error::FailureStage;
use stellatune_audio_core::{
    error::PlaybackControlError, playback::PlaybackItem, source::SourceOpenPurpose,
};

/// Replaces or clears the successor without replacing the active session.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct SetNext {
    pub(in crate::playback) item: Option<PlaybackItem>,
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
                    FailureStage::Planner,
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
