//! Requests a bounded final-output gain ramp.

use super::super::PlaybackActor;
use super::ControlResult;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::playback::MediaTime;

/// Requests a bounded final-output gain ramp.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct SetOutputGain {
    pub(in crate::playback) gain: f32,
    pub(in crate::playback) ramp: MediaTime,
}

impl Responder<SetOutputGain> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        request: SetOutputGain,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        self.session.output_gain = request.gain;
        let result = match self.session.current.as_mut() {
            Some(current) => current.output.set_gain(
                request.gain,
                request.ramp.to_frames(current.output_format.sample_rate),
            ),
            None => Ok(()),
        };
        let _ = reply_to.send(result);
        Ok(())
    }
}
