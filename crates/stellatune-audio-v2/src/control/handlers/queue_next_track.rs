use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::QueueNextTrackMessage;

impl Handler<QueueNextTrackMessage> for ControlActor {
    fn handle(
        &mut self,
        message: QueueNextTrackMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.queue_next(message.track_token, timeout)
    }
}
