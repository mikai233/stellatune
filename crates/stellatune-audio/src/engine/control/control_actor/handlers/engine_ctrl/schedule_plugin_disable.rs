use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tokio::sync::oneshot::Sender as OneshotSender;

use super::super::super::ControlActor;

pub(crate) struct SchedulePluginDisableMessage {
    pub(crate) plugin_id: String,
    pub(crate) resp_tx: OneshotSender<Result<bool, String>>,
}

impl Message for SchedulePluginDisableMessage {
    type Response = ();
}

impl Handler<SchedulePluginDisableMessage> for ControlActor {
    fn handle(&mut self, message: SchedulePluginDisableMessage, _ctx: &mut ActorContext<Self>) {
        let plugin_id = message.plugin_id.trim().to_string();
        let result = if plugin_id.is_empty() {
            Err("plugin_id is empty".to_string())
        } else {
            self.state.pending_disable_plugins.insert(plugin_id);
            Ok(true)
        };
        let _ = message.resp_tx.send(result);
    }
}
