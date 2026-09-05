//! Replaces policies captured by future executable plans.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::planner::PlaybackPolicies;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

/// Replaces policies captured by future executable plans.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct SetPolicies {
    pub(in crate::playback) policies: PlaybackPolicies,
}

impl Responder<SetPolicies> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        request: SetPolicies,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        self.session.policies = request.policies;
        let _ = reply_to.send(Ok(()));
        Ok(())
    }
}
