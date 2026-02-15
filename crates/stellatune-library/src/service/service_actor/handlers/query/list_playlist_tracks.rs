use super::{ActorContext, Handler, LibraryServiceActor, Message, TrackLite};

pub(crate) struct ListPlaylistTracksMessage {
    pub(crate) playlist_id: i64,
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Message for ListPlaylistTracksMessage {
    type Response = Result<Vec<TrackLite>, String>;
}

#[async_trait::async_trait]
impl Handler<ListPlaylistTracksMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: ListPlaylistTracksMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<TrackLite>, String> {
        self.worker
            .list_playlist_tracks(
                message.playlist_id,
                message.query,
                message.limit,
                message.offset,
            )
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
