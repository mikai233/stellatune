use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::SetLfeModeMessage;

impl Handler<SetLfeModeMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetLfeModeMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.set_lfe_mode(message.mode, timeout)
    }
}
