use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tokio::sync::mpsc;

use crate::runtime::actor::PluginRuntimeActor;
use crate::runtime::backend_control::BackendControlRequest;

pub(crate) struct SubscribeBackendControlRequestsMessage;

impl Message for SubscribeBackendControlRequestsMessage {
    type Response = mpsc::UnboundedReceiver<BackendControlRequest>;
}

impl Handler<SubscribeBackendControlRequestsMessage> for PluginRuntimeActor {
    fn handle(
        &mut self,
        _message: SubscribeBackendControlRequestsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> mpsc::UnboundedReceiver<BackendControlRequest> {
        self.event_bus.subscribe_control_requests()
    }
}
