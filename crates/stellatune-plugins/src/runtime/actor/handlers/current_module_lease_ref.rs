use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::model::ModuleLeaseRef;

pub(crate) struct CurrentModuleLeaseRefMessage {
    pub plugin_id: String,
}

impl Message for CurrentModuleLeaseRefMessage {
    type Response = Option<ModuleLeaseRef>;
}

impl Handler<CurrentModuleLeaseRefMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: CurrentModuleLeaseRefMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Option<ModuleLeaseRef> {
        let slot = self.modules.get(&message.plugin_id)?;
        let current = slot.current.as_ref()?;
        Some(ModuleLeaseRef::from_lease(&current.lease))
    }
}
