use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::config::engine::PlaybackState;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::PlayMessage;
use crate::error::EngineError;

impl Responder<PlayMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: PlayMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        if *ctx.behavior() == PlaybackState::Reconfiguring {
            let _ = reply_to.send(Err(EngineError::PluginChangeInProgress {
                operation: "play",
            }));
            return Ok(());
        }
        let timeout = self.config.decode_command_timeout;
        let result = self
            .ensure_session()
            .and_then(|worker| worker.play(timeout).map_err(EngineError::from));
        if result.is_ok() {
            self.transition_state(ctx, PlaybackState::Playing);
            self.schedule_pump(ctx, std::time::Duration::ZERO);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}
