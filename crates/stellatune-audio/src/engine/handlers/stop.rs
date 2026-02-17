use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::config::engine::PlayerState;
use crate::engine::actor::ControlActor;
use crate::engine::messages::StopMessage;

impl Handler<StopMessage> for ControlActor {
    fn handle(
        &mut self,
        message: StopMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.stop(message.behavior, timeout)?;
        self.snapshot.current_track = None;
        self.update_state(PlayerState::Stopped);
        self.update_position(0);
        Ok(())
    }
}
