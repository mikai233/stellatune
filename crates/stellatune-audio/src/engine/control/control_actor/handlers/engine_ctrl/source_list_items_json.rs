use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tokio::sync::oneshot::Sender as OneshotSender;

use super::super::super::super::source_list_items_json_via_runtime_async;
use super::super::super::ControlActor;

pub(crate) struct SourceListItemsJsonMessage {
    pub(crate) plugin_id: String,
    pub(crate) type_id: String,
    pub(crate) config_json: String,
    pub(crate) request_json: String,
    pub(crate) resp_tx: OneshotSender<Result<String, String>>,
}

impl Message for SourceListItemsJsonMessage {
    type Response = ();
}

impl Handler<SourceListItemsJsonMessage> for ControlActor {
    fn handle(&mut self, message: SourceListItemsJsonMessage, _ctx: &mut ActorContext<Self>) {
        source_list_items_json_via_runtime_async(
            message.plugin_id,
            message.type_id,
            message.config_json,
            message.request_json,
            message.resp_tx,
        );
    }
}
