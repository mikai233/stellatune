use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::SeekMessage;

impl Handler<SeekMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SeekMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.seek(message.position_ms, timeout)?;
        self.update_position(message.position_ms);
        Ok(())
    }
}
