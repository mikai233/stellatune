use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tokio::sync::oneshot::Sender as OneshotSender;

use super::super::super::super::lyrics_fetch_json_via_runtime_async;
use super::super::super::ControlActor;

pub(crate) struct LyricsFetchJsonMessage {
    pub(crate) plugin_id: String,
    pub(crate) type_id: String,
    pub(crate) track_json: String,
    pub(crate) resp_tx: OneshotSender<Result<String, String>>,
}

impl Message for LyricsFetchJsonMessage {
    type Response = ();
}

impl Handler<LyricsFetchJsonMessage> for ControlActor {
    fn handle(&mut self, message: LyricsFetchJsonMessage, _ctx: &mut ActorContext<Self>) {
        lyrics_fetch_json_via_runtime_async(
            message.plugin_id,
            message.type_id,
            message.track_json,
            message.resp_tx,
        );
    }
}
