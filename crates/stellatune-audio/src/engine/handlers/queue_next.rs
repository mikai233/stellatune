use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::QueueNextTrackMessage;
use crate::error::EngineError;

impl Handler<QueueNextTrackMessage> for ControlActor {
    fn handle(
        &mut self,
        message: QueueNextTrackMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker
            .queue_next(message.track_token, timeout)
            .map_err(EngineError::from)
    }
}
