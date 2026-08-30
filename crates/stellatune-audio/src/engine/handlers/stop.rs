use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::config::engine::PlaybackState;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::StopMessage;
use crate::error::EngineError;

impl Responder<StopMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: StopMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        let previous = *ctx.behavior();
        self.transition_state(ctx, PlaybackState::Stopping);
        let timeout = self.config.decode_command_timeout;
        let result = self.ensure_session().and_then(|worker| {
            worker
                .stop(message.behavior, timeout)
                .map_err(EngineError::from)
        });
        if result.is_ok() {
            self.pump_scheduled = false;
            self.plugin_checkpoint = None;
            self.current_track = None;
            self.update_position(0);
            self.transition_state(ctx, PlaybackState::Idle);
        } else {
            self.transition_state(ctx, previous);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}
