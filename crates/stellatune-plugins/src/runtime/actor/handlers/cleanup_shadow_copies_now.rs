use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct CleanupShadowCopiesNowMessage;

impl Message for CleanupShadowCopiesNowMessage {
    type Response = ();
}

impl Handler<CleanupShadowCopiesNowMessage> for PluginRuntimeActor {
    fn handle(&mut self, _message: CleanupShadowCopiesNowMessage, _ctx: &mut ActorContext<Self>) {
        self.cleanup_shadow_copies_best_effort("cleanup_shadow_copies_now");
    }
}
