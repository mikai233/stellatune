use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::ApplyPipelineMutationMessage;

impl Handler<ApplyPipelineMutationMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyPipelineMutationMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.apply_pipeline_mutation(message.mutation, timeout)
    }
}
