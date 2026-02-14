use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::RuntimeRouterActor;

pub(crate) struct SetEngineMessage {
    pub(crate) engine: stellatune_audio::EngineHandle,
}

impl Message for SetEngineMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<SetEngineMessage> for RuntimeRouterActor {
    async fn handle(&mut self, message: SetEngineMessage, _ctx: &mut ActorContext<Self>) -> () {
        self.engine = Some(message.engine);
    }
}
