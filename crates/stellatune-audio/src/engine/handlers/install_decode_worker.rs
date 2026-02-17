use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::engine::actor::ControlActor;
use crate::engine::messages::InstallDecodeWorkerMessage;

impl Handler<InstallDecodeWorkerMessage> for ControlActor {
    fn handle(
        &mut self,
        message: InstallDecodeWorkerMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        if self.worker.is_some() {
            return Err("decode worker already installed".to_string());
        }
        self.worker = Some(message.worker);
        Ok(())
    }
}
