use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::DecodeCtrl;
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct SetLfeModeMessage {
    pub(crate) mode: crate::types::LfeMode,
}

impl Message for SetLfeModeMessage {
    type Response = ();
}

impl Handler<SetLfeModeMessage> for ControlActor {
    fn handle(&mut self, message: SetLfeModeMessage, _ctx: &mut ActorContext<Self>) {
        self.state.lfe_mode = message.mode;
        if let Some(session) = self.state.session.as_ref() {
            let _ = session
                .ctrl_tx
                .send(DecodeCtrl::SetLfeMode { mode: message.mode });
        }
    }
}
