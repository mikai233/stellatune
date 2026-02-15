use super::{ActorContext, Handler, LibraryServiceActor, Message};

pub(crate) struct ListFoldersMessage;

impl Message for ListFoldersMessage {
    type Response = Result<Vec<String>, String>;
}

#[async_trait::async_trait]
impl Handler<ListFoldersMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListFoldersMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<String>, String> {
        self.worker
            .list_folders()
            .await
            .map_err(|e| format!("{e:#}"))
    }
}
