use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::ApplyStageRuntimeUpdateMessage;
use crate::error::EngineError;

impl Handler<ApplyStageRuntimeUpdateMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyStageRuntimeUpdateMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.apply_stage_runtime_update(message.target, message.update, timeout)?;
        Ok(())
    }
}
