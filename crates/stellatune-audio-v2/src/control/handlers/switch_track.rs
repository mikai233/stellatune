use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::SwitchTrackMessage;
use crate::types::PlayerState;

impl Handler<SwitchTrackMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SwitchTrackMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.open(message.track_token, message.autoplay, timeout)?;
        self.update_state(if message.autoplay {
            PlayerState::Playing
        } else {
            PlayerState::Paused
        });
        Ok(())
    }
}
