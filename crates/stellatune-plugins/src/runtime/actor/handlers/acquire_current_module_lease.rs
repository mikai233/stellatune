use std::sync::Arc;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::model::ModuleLease;

pub(crate) struct AcquireCurrentModuleLeaseMessage {
    pub plugin_id: String,
}

impl Message for AcquireCurrentModuleLeaseMessage {
    type Response = Option<Arc<ModuleLease>>;
}

impl Handler<AcquireCurrentModuleLeaseMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: AcquireCurrentModuleLeaseMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Option<Arc<ModuleLease>> {
        if self.disabled_plugin_ids.contains(&message.plugin_id) {
            return None;
        }
        let slot = self.modules.get(&message.plugin_id)?;
        slot.current.as_ref().cloned()
    }
}
