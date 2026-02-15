use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tokio::sync::oneshot::Sender as OneshotSender;

use super::super::super::super::output_sink_list_targets_json_via_runtime;
use super::super::super::ControlActor;

pub(crate) struct OutputSinkListTargetsJsonMessage {
    pub(crate) plugin_id: String,
    pub(crate) type_id: String,
    pub(crate) config_json: String,
    pub(crate) resp_tx: OneshotSender<Result<String, String>>,
}

impl Message for OutputSinkListTargetsJsonMessage {
    type Response = ();
}

impl Handler<OutputSinkListTargetsJsonMessage> for ControlActor {
    fn handle(&mut self, message: OutputSinkListTargetsJsonMessage, _ctx: &mut ActorContext<Self>) {
        let _ = message
            .resp_tx
            .send(output_sink_list_targets_json_via_runtime(
                &mut self.state,
                &message.plugin_id,
                &message.type_id,
                message.config_json,
            ));
    }
}
