use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct BroadcastHostEventJsonMessage {
    pub event_json: String,
}

impl Message for BroadcastHostEventJsonMessage {
    type Response = ();
}

impl Handler<BroadcastHostEventJsonMessage> for PluginRuntimeActor {
    fn handle(&mut self, message: BroadcastHostEventJsonMessage, _ctx: &mut ActorContext<Self>) {
        if message.event_json.is_empty() {
            return;
        }
        for plugin_id in self.event_bus.registered_plugin_ids() {
            self.event_bus
                .push_host_event(&plugin_id, message.event_json.clone());
        }
    }
}
