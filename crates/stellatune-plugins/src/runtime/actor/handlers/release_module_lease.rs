use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct ReleaseModuleLeaseMessage {
    pub plugin_id: String,
    pub lease_id: u64,
}

impl Message for ReleaseModuleLeaseMessage {
    type Response = ();
}

impl Handler<ReleaseModuleLeaseMessage> for PluginRuntimeActor {
    fn handle(&mut self, message: ReleaseModuleLeaseMessage, _ctx: &mut ActorContext<Self>) {
        self.release_module_lease_ref(&message.plugin_id, message.lease_id);
    }
}
