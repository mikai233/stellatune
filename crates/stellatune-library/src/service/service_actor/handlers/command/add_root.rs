use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct AddRootMessage {
    pub(crate) path: String,
}

impl Message for AddRootMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<AddRootMessage> for LibraryServiceActor {
    async fn handle(&mut self, message: AddRootMessage, _ctx: &mut ActorContext<Self>) -> () {
        if let Err(err) = self.worker.add_root(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
