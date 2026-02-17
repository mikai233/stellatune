use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::config::engine::PlayerState;
use crate::engine::actor::ControlActor;
use crate::engine::messages::PlayMessage;
use crate::error::EngineError;

impl Handler<PlayMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: PlayMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.play(timeout)?;
        self.update_state(PlayerState::Playing);
        Ok(())
    }
}
