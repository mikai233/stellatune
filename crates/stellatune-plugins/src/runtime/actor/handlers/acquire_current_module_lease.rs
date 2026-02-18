use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::model::AcquiredModuleLease;

pub(crate) struct AcquireCurrentModuleLeaseMessage {
    pub plugin_id: String,
}

impl Message for AcquireCurrentModuleLeaseMessage {
    type Response = Option<AcquiredModuleLease>;
}

impl Handler<AcquireCurrentModuleLeaseMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: AcquireCurrentModuleLeaseMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Option<AcquiredModuleLease> {
        if self.disabled_plugin_ids.contains(&message.plugin_id) {
            return None;
        }
        let slot = self.modules.get_mut(&message.plugin_id)?;
        slot.acquire_current()
    }
}
