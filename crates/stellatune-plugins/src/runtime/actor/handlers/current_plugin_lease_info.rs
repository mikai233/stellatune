use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::introspection::PluginLeaseInfo;

pub(crate) struct CurrentPluginLeaseInfoMessage {
    pub plugin_id: String,
}

impl Message for CurrentPluginLeaseInfoMessage {
    type Response = Option<PluginLeaseInfo>;
}

impl Handler<CurrentPluginLeaseInfoMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: CurrentPluginLeaseInfoMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Option<PluginLeaseInfo> {
        let slot = self.modules.get(&message.plugin_id)?;
        let lease = &slot.current.as_ref()?.lease;
        Some(PluginLeaseInfo {
            lease_id: lease.lease_id,
            metadata_json: lease.metadata_json.clone(),
        })
    }
}
