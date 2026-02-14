use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct CollectRetiredModuleLeasesByRefcountMessage;

impl Message for CollectRetiredModuleLeasesByRefcountMessage {
    type Response = usize;
}

impl Handler<CollectRetiredModuleLeasesByRefcountMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        _message: CollectRetiredModuleLeasesByRefcountMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> usize {
        self.collect_retired_module_leases_by_refcount()
    }
}
