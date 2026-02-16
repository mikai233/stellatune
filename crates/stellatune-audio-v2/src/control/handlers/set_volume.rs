use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::SetVolumeMessage;

impl Handler<SetVolumeMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetVolumeMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.set_master_gain_level(message.volume, timeout)?;
        Ok(())
    }
}
