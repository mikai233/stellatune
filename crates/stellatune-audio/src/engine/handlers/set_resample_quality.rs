use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::engine::actor::PlaybackActor;
use crate::engine::messages::SetResampleQualityMessage;
use crate::error::EngineError;

impl Responder<SetResampleQualityMessage> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: SetResampleQualityMessage,
        reply_to: ReplyTo<Result<(), EngineError>>,
    ) -> Result<(), ActorError> {
        let timeout = self.config.decode_command_timeout;
        let result = self.ensure_session().and_then(|worker| {
            worker
                .set_resample_quality(message.quality, timeout)
                .map_err(EngineError::from)
        });
        let _ = reply_to.send(result);
        Ok(())
    }
}
