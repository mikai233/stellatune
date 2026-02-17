use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::config::engine::PlayerState;
use crate::engine::actor::ControlActor;
use crate::engine::messages::ShutdownMessage;

impl Handler<ShutdownMessage> for ControlActor {
    fn handle(
        &mut self,
        _message: ShutdownMessage,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        if let Some(worker) = self.worker.take() {
            worker.shutdown(self.config.decode_command_timeout)?;
        }
        self.snapshot.current_track = None;
        self.update_position(0);
        self.update_state(PlayerState::Stopped);
        ctx.stop();
        Ok(())
    }
}
