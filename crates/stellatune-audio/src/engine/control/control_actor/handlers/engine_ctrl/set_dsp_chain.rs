use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{Event, apply_dsp_chain, parse_dsp_chain};

pub(crate) struct SetDspChainMessage {
    pub(crate) chain: Vec<crate::types::DspChainItem>,
}

impl Message for SetDspChainMessage {
    type Response = ();
}

impl Handler<SetDspChainMessage> for ControlActor {
    fn handle(&mut self, message: SetDspChainMessage, _ctx: &mut ActorContext<Self>) {
        let parsed = match parse_dsp_chain(message.chain) {
            Ok(parsed) => parsed,
            Err(message) => {
                self.events.emit(Event::Error { message });
                return;
            },
        };
        self.state.desired_dsp_chain = parsed;
        if self.state.session.is_some()
            && let Err(message) = apply_dsp_chain(&mut self.state)
        {
            self.events.emit(Event::Error { message });
        }
    }
}
