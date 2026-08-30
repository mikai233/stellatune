use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::engine::actor::PlaybackActor;
use crate::engine::messages::SeekMessage;
use crate::error::EngineError;

impl Responder<SeekMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: SeekMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        if *ctx.behavior() == crate::config::engine::PlaybackState::Reconfiguring {
            let _ = reply_to.send(Err(EngineError::PluginChangeInProgress {
                operation: "seek_ms",
            }));
            return Ok(());
        }
        let timeout = self.config.decode_command_timeout;
        let result = self.ensure_session().and_then(|worker| {
            worker
                .seek(message.position_ms, timeout)
                .map_err(EngineError::from)
        });
        if result.is_ok() {
            self.update_position(message.position_ms);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}
