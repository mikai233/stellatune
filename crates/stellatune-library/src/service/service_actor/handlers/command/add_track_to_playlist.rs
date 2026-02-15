use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct AddTrackToPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_id: i64,
}

impl Message for AddTrackToPlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<AddTrackToPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: AddTrackToPlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self
            .worker
            .add_track_to_playlist(message.playlist_id, message.track_id)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
