use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct AddTracksToPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_ids: Vec<i64>,
}

impl Message for AddTracksToPlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<AddTracksToPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: AddTracksToPlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self
            .worker
            .add_tracks_to_playlist(message.playlist_id, message.track_ids)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
