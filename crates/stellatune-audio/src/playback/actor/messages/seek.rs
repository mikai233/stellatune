//! Requests an absolute media seek on the current item.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::playback::{
    event::{PlaybackEvent, PlaybackState},
    lifecycle::{finish_seek, set_state, start_seek},
    state::PendingSeek,
};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::{
    decoder::DecoderSeekStatus, error::PlaybackControlError, playback::MediaTime,
};

/// Requests an absolute media seek on the current item.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct Seek {
    pub(in crate::playback) position: MediaTime,
}

impl Responder<Seek> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        request: Seek,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        if let Some(pending) = self.session.pending_seek.take() {
            let _ = pending.response.send(Err(PlaybackControlError::Closed));
        }
        let mut state = *ctx.behavior();
        match start_seek(&mut self.session, request.position) {
            Ok((_item_id, DecoderSeekStatus::Complete(result))) => {
                finish_seek(&mut self.session, result, &self.event_tx);
                self.session
                    .current
                    .as_ref()
                    .unwrap()
                    .output
                    .reply_when_settled(reply_to);
            },
            Ok((item_id, DecoderSeekStatus::Pending)) => {
                set_state(&mut state, PlaybackState::Buffering, &self.event_tx);
                let _ = self.event_tx.send(PlaybackEvent::Buffering {
                    item_id,
                    active: true,
                });
                self.session.pending_seek = Some(PendingSeek {
                    response: reply_to,
                    item_id,
                });
            },
            Err(error) => {
                let _ = reply_to.send(Err(error));
            },
        }
        self.transition(ctx, state);
        Ok(())
    }
}
