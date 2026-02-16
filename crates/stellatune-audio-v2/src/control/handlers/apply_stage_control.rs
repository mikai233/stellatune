use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::ApplyStageControlMessage;

impl Handler<ApplyStageControlMessage> for ControlActor {
    fn handle(
        &mut self,
        message: ApplyStageControlMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.apply_stage_control(message.stage_key, message.control, timeout)?;
        Ok(())
    }
}
