use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct SetPluginEnabledMessage {
    pub plugin_id: String,
    pub enabled: bool,
}

impl Message for SetPluginEnabledMessage {
    type Response = ();
}

impl Handler<SetPluginEnabledMessage> for PluginRuntimeActor {
    fn handle(&mut self, message: SetPluginEnabledMessage, _ctx: &mut ActorContext<Self>) -> () {
        let plugin_id = message.plugin_id.trim();
        if plugin_id.is_empty() {
            return;
        }
        if message.enabled {
            self.disabled_plugin_ids.remove(plugin_id);
        } else {
            self.disabled_plugin_ids.insert(plugin_id.to_string());
        }
    }
}
