use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::introspection::{PluginLeaseInfo, PluginLeaseState};

pub(crate) struct PluginLeaseStateMessage {
    pub plugin_id: String,
}

impl Message for PluginLeaseStateMessage {
    type Response = Option<PluginLeaseState>;
}

impl Handler<PluginLeaseStateMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: PluginLeaseStateMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Option<PluginLeaseState> {
        let slot = self.modules.get(&message.plugin_id)?;
        let current = slot.current.as_ref().map(|entry| PluginLeaseInfo {
            lease_id: entry.lease.lease_id,
            metadata_json: entry.lease.metadata_json.clone(),
        });
        let retired_lease_ids = slot
            .retired
            .iter()
            .map(|entry| entry.lease.lease_id)
            .collect::<Vec<_>>();
        Some(PluginLeaseState {
            current,
            retired_lease_ids,
        })
    }
}
