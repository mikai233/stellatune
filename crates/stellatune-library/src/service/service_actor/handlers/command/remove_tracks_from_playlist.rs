use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct RemoveTracksFromPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_ids: Vec<i64>,
}

impl Message for RemoveTracksFromPlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<RemoveTracksFromPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: RemoveTracksFromPlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self
            .worker
            .remove_tracks_from_playlist(message.playlist_id, message.track_ids)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
