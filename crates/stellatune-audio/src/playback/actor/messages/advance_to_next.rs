//! Claims a specific successor atomically, including its in-flight preparation.

use super::super::PlaybackActor;
use crate::playback::control::{AdvanceOutcome, SwitchOptions};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::{error::PlaybackControlError, playback::PlaybackItemId};
type AdvanceResult = Result<AdvanceOutcome, PlaybackControlError>;

/// Claims a specific successor atomically, including its in-flight preparation.
#[derive(lattice_actor::Request)]
#[request(response = AdvanceResult)]
pub(in crate::playback) struct AdvanceToNext {
    pub(in crate::playback) expected_item_id: PlaybackItemId,
    pub(in crate::playback) options: SwitchOptions,
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
            self.session.wants_playing = request.options.autoplay;
            let mut state = *ctx.behavior();
            let result = self
                .apply_overlap_intent(&mut state, request.options)
                .map(|()| AdvanceOutcome::Accepted);
            self.transition(ctx, state);
            let _ = reply_to.send(result);
            return Ok(());
        }
        self.session.wants_playing = request.options.autoplay;
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
