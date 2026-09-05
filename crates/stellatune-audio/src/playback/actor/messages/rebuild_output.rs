//! Recreates the active sink for the current route configuration.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::playback::{event::PlaybackState, sink_worker::SinkWorker};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use std::sync::Arc;

/// Recreates the active sink for the current route configuration.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct RebuildOutput;

impl Responder<RebuildOutput> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: RebuildOutput,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let should_resume = *ctx.behavior() == PlaybackState::Playing;
        let output_gain = self.session.output_gain;
        let result = match self.session.current.as_mut() {
            Some(current) => (|| {
                current.output.shutdown();
                current.output = SinkWorker::start(
                    Arc::clone(&current.sink_factory),
                    current.output_format,
                    self.config.pcm_ring_blocks,
                    output_gain,
                )?;
                if should_resume {
                    current.output.resume()?;
                }
                Ok(())
            })(),
            None => Ok(()),
        };
        let _ = reply_to.send(result);
        Ok(())
    }
}
