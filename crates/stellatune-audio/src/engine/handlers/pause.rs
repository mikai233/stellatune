use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::config::engine::PlaybackState;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::PauseMessage;
use crate::error::EngineError;

impl Responder<PauseMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: PauseMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        if *ctx.behavior() == PlaybackState::Reconfiguring {
            let _ = reply_to.send(Err(EngineError::PluginChangeInProgress {
                operation: "pause",
            }));
            return Ok(());
        }
        let timeout = self.config.decode_command_timeout;
        let result = self.ensure_session().and_then(|worker| {
            worker
                .pause(message.behavior, timeout)
                .map_err(EngineError::from)
        });
        if result.is_ok() {
            self.pump_scheduled = false;
            self.transition_state(ctx, PlaybackState::Paused);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}
