use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::ApplyPipelineMutationsMessage;
use crate::error::EngineError;

impl Handler<ApplyPipelineMutationsMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyPipelineMutationsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker
            .apply_pipeline_mutations(message.mutations, timeout)
            .map_err(EngineError::from)
    }
}
