use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;

pub(crate) struct ActivePluginIdsMessage;

impl Message for ActivePluginIdsMessage {
    type Response = Vec<String>;
}

impl Handler<ActivePluginIdsMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        _message: ActivePluginIdsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Vec<String> {
        self.modules
            .iter()
            .filter(|(_, slot)| slot.current.is_some())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect()
    }
}
