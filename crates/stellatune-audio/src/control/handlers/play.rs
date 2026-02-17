use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::PlayMessage;
use crate::types::PlayerState;

impl Handler<PlayMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: PlayMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.play(timeout)?;
        self.update_state(PlayerState::Playing);
        Ok(())
    }
}
