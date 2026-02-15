use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct RemoveTrackFromPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_id: i64,
}

impl Message for RemoveTrackFromPlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<RemoveTrackFromPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: RemoveTrackFromPlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self
            .worker
            .remove_track_from_playlist(message.playlist_id, message.track_id)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
