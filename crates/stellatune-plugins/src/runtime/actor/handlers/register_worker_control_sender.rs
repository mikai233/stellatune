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
        let plugin_id = message.plugin_id;
        let sender = message.sender;
        let subscribers = self
            .worker_control_subscribers
            .entry(plugin_id.clone())
            .or_default();
        subscribers.push(sender);
        tracing::debug!(
            plugin_id,
            subscribers = subscribers.len(),
            "worker control subscriber registered"
        );
        true
    }
}
