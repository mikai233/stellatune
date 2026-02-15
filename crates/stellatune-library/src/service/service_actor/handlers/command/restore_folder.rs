use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct RestoreFolderMessage {
    pub(crate) path: String,
}

impl Message for RestoreFolderMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<RestoreFolderMessage> for LibraryServiceActor {
    async fn handle(&mut self, message: RestoreFolderMessage, _ctx: &mut ActorContext<Self>) -> () {
        if let Err(err) = self.worker.restore_folder(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
