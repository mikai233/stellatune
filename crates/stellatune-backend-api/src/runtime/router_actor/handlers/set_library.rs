use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::RuntimeRouterActor;

pub(crate) struct SetLibraryMessage {
    pub(crate) library: stellatune_library::LibraryHandle,
}

impl Message for SetLibraryMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<SetLibraryMessage> for RuntimeRouterActor {
    async fn handle(&mut self, message: SetLibraryMessage, _ctx: &mut ActorContext<Self>) -> () {
        self.library = Some(message.library);
    }
}
