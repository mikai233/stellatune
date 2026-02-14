use std::collections::HashSet;

use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct SetDisabledPluginIdsMessage {
    pub disabled_ids: HashSet<String>,
}

impl Message for SetDisabledPluginIdsMessage {
    type Response = ();
}

impl Handler<SetDisabledPluginIdsMessage> for PluginRuntimeActor {
    fn handle(&mut self, message: SetDisabledPluginIdsMessage, _ctx: &mut ActorContext<Self>) {
        self.disabled_plugin_ids = message.disabled_ids;
    }
}
