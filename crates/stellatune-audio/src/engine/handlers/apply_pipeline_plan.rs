use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::ApplyPipelinePlanMessage;
use crate::error::EngineError;

impl Handler<ApplyPipelinePlanMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyPipelinePlanMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker
            .apply_pipeline_plan(message.plan, timeout)
            .map_err(EngineError::from)
    }
}
