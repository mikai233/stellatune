use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::engine::actor::PlaybackActor;
use crate::engine::messages::QueueNextTrackMessage;
use crate::error::EngineError;

impl Responder<QueueNextTrackMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: QueueNextTrackMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        if *ctx.behavior() == crate::config::engine::PlaybackState::Reconfiguring {
            let _ = reply_to.send(Err(EngineError::PluginChangeInProgress {
                operation: "queue_next_track_token",
            }));
            return Ok(());
        }
        let timeout = self.config.decode_command_timeout;
        let result = self.ensure_session().and_then(|worker| {
            worker
                .queue_next(message.track_token, timeout)
                .map_err(EngineError::from)
        });
        let _ = reply_to.send(result);
        Ok(())
    }
}
