use std::cell::RefCell;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tracing::info;

use crate::engine::control::control_actor::ControlActor;

thread_local! {
    static CONTROL_RT_GUARD: RefCell<Option<crate::output::RealtimeThreadGuard>> =
        const { RefCell::new(None) };
}

pub(crate) struct ControlInitMessage;

impl Message for ControlInitMessage {
    type Response = ();
}

impl Handler<ControlInitMessage> for ControlActor {
    fn handle(&mut self, _message: ControlInitMessage, _ctx: &mut ActorContext<Self>) {
        CONTROL_RT_GUARD.with(|guard| {
            if guard.borrow().is_none() {
                *guard.borrow_mut() = Some(crate::output::enable_realtime_audio_thread());
                info!("control thread started");
            }
        });
    }
}
