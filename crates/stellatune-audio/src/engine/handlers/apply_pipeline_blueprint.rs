use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::ApplyPipelineBlueprintMessage;
use crate::error::EngineError;

impl Handler<ApplyPipelineBlueprintMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyPipelineBlueprintMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker
            .apply_pipeline_blueprint(message.blueprint, timeout)
            .map_err(EngineError::from)
    }
}
