//! Requests output playback or resumption.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::playback::{event::PlaybackState, lifecycle::set_state};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::error::PlaybackControlError;

/// Requests output playback or resumption.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct Play;

impl Responder<Play> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: Play,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let result = self
            .session
            .current
            .as_mut()
            .ok_or(PlaybackControlError::InvalidState)
            .and_then(|current| current.output.resume());
        if result.is_ok() {
            if let Some(options) = self.session.advance_options.as_mut() {
                options.autoplay = true;
            }
            let mut state = *ctx.behavior();
            set_state(&mut state, PlaybackState::Playing, &self.event_tx);
            self.transition(ctx, state);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}
