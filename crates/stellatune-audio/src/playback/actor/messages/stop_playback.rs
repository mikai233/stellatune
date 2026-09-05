//! Requests teardown of the current playback session state.

use super::super::PlaybackActor;
use super::super::preparation::advance_generation;
use super::ControlResult;
use crate::playback::{
    event::PlaybackState,
    lifecycle::{reject_pending, set_state, stop_current},
};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

/// Requests teardown of the current playback session state.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct StopPlayback;

impl Responder<StopPlayback> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: StopPlayback,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        advance_generation(&mut self.session);
        reject_pending(&mut self.session);
        self.session.crossfade = None;
        self.session.force_transition = false;
        stop_current(&mut self.session);
        self.session.next.clear();
        self.session.next.clear();
        let mut state = *ctx.behavior();
        set_state(&mut state, PlaybackState::Idle, &self.event_tx);
        self.transition(ctx, state);
        let _ = reply_to.send(Ok(()));
        Ok(())
    }
}
