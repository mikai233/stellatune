use super::{ActorContext, Handler, LibraryServiceActor, Message};

pub(crate) struct ListExcludedFoldersMessage;

impl Message for ListExcludedFoldersMessage {
    type Response = Result<Vec<String>, String>;
}

#[async_trait::async_trait]
impl Handler<ListExcludedFoldersMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListExcludedFoldersMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<String>, String> {
        self.worker
            .list_excluded_folders()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
