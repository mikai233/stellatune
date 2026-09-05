//! Stores buffering budgets for subsequent output sessions.
use super::{super::PlaybackActor, ControlResult};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::buffering::LatencyProfile;

#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct SetBuffering {
    pub(in crate::playback) profile: LatencyProfile,
}
impl Responder<SetBuffering> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        request: SetBuffering,
        reply: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        self.config.buffering = request.profile.buffering();
        let _ = reply.send(Ok(()));
        Ok(())
    }
}
