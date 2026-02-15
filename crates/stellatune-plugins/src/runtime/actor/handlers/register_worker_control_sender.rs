use crossbeam_channel::Sender;
use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::messages::WorkerControlMessage;

pub(crate) struct RegisterWorkerControlSenderMessage {
    pub plugin_id: String,
    pub sender: Sender<WorkerControlMessage>,
}

impl Message for RegisterWorkerControlSenderMessage {
    type Response = bool;
}

impl Handler<RegisterWorkerControlSenderMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        message: RegisterWorkerControlSenderMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> bool {
        self.worker_control_subscribers
            .entry(message.plugin_id)
            .or_default()
            .push(message.sender);
        true
    }
}
