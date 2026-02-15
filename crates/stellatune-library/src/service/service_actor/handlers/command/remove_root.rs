use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct RemoveRootMessage {
    pub(crate) path: String,
}

impl Message for RemoveRootMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<RemoveRootMessage> for LibraryServiceActor {
    async fn handle(&mut self, message: RemoveRootMessage, _ctx: &mut ActorContext<Self>) -> () {
        if let Err(err) = self.worker.remove_root(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
