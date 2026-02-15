use super::{ActorContext, Handler, LibraryServiceActor, Message, PlaylistLite};

pub(crate) struct ListPlaylistsMessage;

impl Message for ListPlaylistsMessage {
    type Response = Result<Vec<PlaylistLite>, String>;
}

#[async_trait::async_trait]
impl Handler<ListPlaylistsMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListPlaylistsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<PlaylistLite>, String> {
        self.worker
            .list_playlists()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
