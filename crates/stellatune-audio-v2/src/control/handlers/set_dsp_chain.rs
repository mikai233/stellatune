use stellatune_runtime::thread_actor::{ActorContext, Handler};

use crate::control::actor::ControlActor;
use crate::control::messages::SetDspChainMessage;

impl Handler<SetDspChainMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SetDspChainMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let timeout = self.config.decode_command_timeout;
        let worker = self.ensure_worker()?;
        worker.set_dsp_chain(message.spec, timeout)
    }
}
