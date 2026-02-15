use std::collections::HashSet;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct DisabledPluginIdsMessage;

impl Message for DisabledPluginIdsMessage {
    type Response = HashSet<String>;
}

impl Handler<DisabledPluginIdsMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        _message: DisabledPluginIdsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> HashSet<String> {
        self.disabled_plugin_ids.clone()
    }
}
