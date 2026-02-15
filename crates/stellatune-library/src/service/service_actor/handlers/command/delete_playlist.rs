use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct DeletePlaylistMessage {
    pub(crate) id: i64,
}

impl Message for DeletePlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<DeletePlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: DeletePlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self.worker.delete_playlist(message.id).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
