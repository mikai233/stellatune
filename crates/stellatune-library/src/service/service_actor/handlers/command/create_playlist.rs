use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct CreatePlaylistMessage {
    pub(crate) name: String,
}

impl Message for CreatePlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<CreatePlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: CreatePlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self.worker.create_playlist(message.name).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
