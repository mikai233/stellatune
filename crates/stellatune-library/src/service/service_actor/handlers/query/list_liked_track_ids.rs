use super::{ActorContext, Handler, LibraryServiceActor, Message};

pub(crate) struct ListLikedTrackIdsMessage;

impl Message for ListLikedTrackIdsMessage {
    type Response = Result<Vec<i64>, String>;
}

#[async_trait::async_trait]
impl Handler<ListLikedTrackIdsMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListLikedTrackIdsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<i64>, String> {
        self.worker
            .list_liked_track_ids()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
