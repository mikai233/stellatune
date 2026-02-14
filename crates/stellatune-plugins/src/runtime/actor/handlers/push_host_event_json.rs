use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct PushHostEventJsonMessage {
    pub plugin_id: String,
    pub event_json: String,
}

impl Message for PushHostEventJsonMessage {
    type Response = ();
}

impl Handler<PushHostEventJsonMessage> for PluginRuntimeActor {
    fn handle(&mut self, message: PushHostEventJsonMessage, _ctx: &mut ActorContext<Self>) -> () {
        let plugin_id = message.plugin_id.trim();
        if plugin_id.is_empty() || message.event_json.is_empty() {
            return;
        }
        self.event_bus
            .push_host_event(plugin_id, message.event_json);
    }
}
