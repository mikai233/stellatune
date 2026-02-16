use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::PauseMessage;
use crate::types::PlayerState;

impl Handler<PauseMessage> for ControlActor {
    fn handle(
        &mut self,
        message: PauseMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.pause(message.behavior, timeout)?;
        self.update_state(PlayerState::Paused);
        Ok(())
    }
}
