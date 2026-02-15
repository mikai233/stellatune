use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct DeleteFolderMessage {
    pub(crate) path: String,
}

impl Message for DeleteFolderMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<DeleteFolderMessage> for LibraryServiceActor {
    async fn handle(&mut self, message: DeleteFolderMessage, _ctx: &mut ActorContext<Self>) -> () {
        if let Err(err) = self.worker.delete_folder(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
