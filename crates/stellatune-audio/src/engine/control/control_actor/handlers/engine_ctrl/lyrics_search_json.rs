use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tokio::sync::oneshot::Sender as OneshotSender;

use super::super::super::super::lyrics_search_json_via_runtime_async;
use super::super::super::ControlActor;

pub(crate) struct LyricsSearchJsonMessage {
    pub(crate) plugin_id: String,
    pub(crate) type_id: String,
    pub(crate) query_json: String,
    pub(crate) resp_tx: OneshotSender<Result<String, String>>,
}

impl Message for LyricsSearchJsonMessage {
    type Response = ();
}

impl Handler<LyricsSearchJsonMessage> for ControlActor {
    fn handle(&mut self, message: LyricsSearchJsonMessage, _ctx: &mut ActorContext<Self>) {
        lyrics_search_json_via_runtime_async(
            message.plugin_id,
            message.type_id,
            message.query_json,
            message.resp_tx,
        );
    }
}
