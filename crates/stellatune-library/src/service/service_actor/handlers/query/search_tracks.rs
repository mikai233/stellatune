use super::{ActorContext, Handler, LibraryServiceActor, Message, TrackLite};

pub(crate) struct SearchTracksMessage {
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Message for SearchTracksMessage {
    type Response = Result<Vec<TrackLite>, String>;
}

#[async_trait::async_trait]
impl Handler<SearchTracksMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        message: SearchTracksMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<TrackLite>, String> {
        self.worker
            .search(message.query, message.limit, message.offset)
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
