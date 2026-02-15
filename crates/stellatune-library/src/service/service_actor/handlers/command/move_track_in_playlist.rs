use super::{ActorContext, Handler, LibraryEvent, LibraryServiceActor, Message};

pub(crate) struct MoveTrackInPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_id: i64,
    pub(crate) new_index: i64,
}

impl Message for MoveTrackInPlaylistMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<MoveTrackInPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: MoveTrackInPlaylistMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> () {
        if let Err(err) = self
            .worker
            .move_track_in_playlist(message.playlist_id, message.track_id, message.new_index)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
    }
}
