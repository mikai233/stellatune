use super::{ActorContext, Handler, LibraryServiceActor, Message, TrackLite};

pub(crate) struct ListTracksMessage {
    pub(crate) folder: String,
    pub(crate) recursive: bool,
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Message for ListTracksMessage {
    type Response = Result<Vec<TrackLite>, String>;
}

#[async_trait::async_trait]
impl Handler<ListTracksMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: ListTracksMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<TrackLite>, String> {
        self.worker
            .list_tracks(
                message.folder,
                message.recursive,
                message.query,
                message.limit,
                message.offset,
            )
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
