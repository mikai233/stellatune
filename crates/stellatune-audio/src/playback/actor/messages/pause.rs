//! Requests output pause without discarding PCM.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::playback::{event::PlaybackState, lifecycle::set_state};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::error::PlaybackControlError;

/// Requests output pause without discarding PCM.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct Pause;

impl Responder<Pause> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: Pause,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let result = if self.session.pending_preparation.is_some()
            || self.session.pending_recovery.is_some()
        {
            let _ = reply_to.send(Ok(()));
            Ok(())
        } else if let Some(current) = self.session.current.as_ref() {
            current.output.request_playing(false, reply_to)
        } else {
            let _ = reply_to.send(Err(PlaybackControlError::InvalidState));
            Err(PlaybackControlError::InvalidState)
        };
        if result.is_ok() {
            self.session.wants_playing = false;
            if let Some(options) = self.session.advance_options.as_mut() {
                options.autoplay = false;
            }
            let mut state = *ctx.behavior();
            set_state(&mut state, PlaybackState::Paused, &self.event_tx);
            self.transition(ctx, state);
        }
        Ok(())
    }
}
