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
        let result = if self.session.pending_preparation.is_some()
            || self.session.pending_recovery.is_some()
        {
            let _ = reply_to.send(Ok(()));
            Ok(())
        } else if let Some(current) = self.session.current.as_ref() {
            current.output.request_playing(true, reply_to)
        } else {
            let _ = reply_to.send(Err(PlaybackControlError::InvalidState));
            Err(PlaybackControlError::InvalidState)
        };
        if result.is_ok() {
            self.session.wants_playing = true;
            if let Some(options) = self.session.advance_options.as_mut() {
                options.autoplay = true;
            }
            let mut state = *ctx.behavior();
            if self.session.pending_preparation.is_none() && self.session.pending_recovery.is_none()
            {
                set_state(&mut state, PlaybackState::Playing, &self.event_tx);
            }
            self.transition(ctx, state);
        }
        Ok(())
    }
}
