use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::InstallDecodeWorkerMessage;
use crate::error::EngineError;

impl Handler<InstallDecodeWorkerMessage> for ControlActor {
    fn handle(
        &mut self,
        message: InstallDecodeWorkerMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), EngineError> {
        if self.worker.is_some() {
            return Err(EngineError::WorkerAlreadyInstalled);
        }
        self.worker = Some(message.worker);
        Ok(())
    }
}
