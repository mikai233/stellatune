use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::ApplyPipelineMutationMessage;
use crate::error::EngineError;

impl Handler<ApplyPipelineMutationMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyPipelineMutationMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker
            .apply_pipeline_mutation(message.mutation, timeout)
            .map_err(EngineError::from)
    }
}
