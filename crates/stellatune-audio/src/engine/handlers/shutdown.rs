use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::config::engine::PlaybackState;
use crate::engine::actor::PlaybackActor;
use crate::engine::messages::ShutdownMessage;
use crate::error::EngineError;

impl Responder<ShutdownMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: ShutdownMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        self.transition_state(ctx, PlaybackState::Stopping);
        self.pump_scheduled = false;
        let result = self
            .session
            .take()
            .map(|session| {
                session
                    .shutdown(self.config.decode_command_timeout)
                    .map_err(EngineError::from)
            })
            .unwrap_or(Ok(()));
        if result.is_ok() {
            self.current_track = None;
            self.update_position(0);
            self.transition_state(ctx, PlaybackState::Idle);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}
