use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::ApplyPipelinePlanMessage;

impl Handler<ApplyPipelinePlanMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyPipelinePlanMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.apply_pipeline_plan(message.plan, timeout)
    }
}
