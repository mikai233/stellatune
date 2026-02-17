use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::SetResampleQualityMessage;
use crate::error::EngineError;

impl Handler<SetResampleQualityMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetResampleQualityMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker
            .set_resample_quality(message.quality, timeout)
            .map_err(EngineError::from)
    }
}
