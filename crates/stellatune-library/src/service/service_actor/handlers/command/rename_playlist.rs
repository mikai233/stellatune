use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct RenamePlaylistMessage {
    pub(crate) id: i64,
    pub(crate) name: String,
}

impl Message for RenamePlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<RenamePlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: RenamePlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self.worker.rename_playlist(message.id, message.name).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
