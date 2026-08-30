use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::engine::actor::PlaybackActor;
use crate::engine::messages::RebuildPipelineMessage;
use crate::error::EngineError;

impl Responder<RebuildPipelineMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: RebuildPipelineMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        let timeout = self.config.decode_command_timeout;
        let result = self
            .ensure_session()
            .and_then(|session| session.rebuild_pipeline(timeout).map_err(EngineError::from));
        let _ = reply_to.send(result);
        Ok(())
    }
}
