use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::engine::control::preload_actor::PreloadActor;

pub(crate) struct PreloadShutdownMessage;

impl Message for PreloadShutdownMessage {
    type Response = ();
}

impl Handler<PreloadShutdownMessage> for PreloadActor {
    fn handle(&mut self, _message: PreloadShutdownMessage, ctx: &mut ActorContext<Self>) {
        ctx.stop();
    }
}
