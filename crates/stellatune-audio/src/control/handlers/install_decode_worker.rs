use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::InstallDecodeWorkerMessage;

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
