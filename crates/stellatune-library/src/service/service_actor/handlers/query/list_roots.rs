use super::{ActorContext, Handler, LibraryServiceActor, Message};

pub(crate) struct ListRootsMessage;

impl Message for ListRootsMessage {
    type Response = Result<Vec<String>, String>;
}

#[async_trait::async_trait]
impl Handler<ListRootsMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _message: ListRootsMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<String>, String> {
        self.worker.list_roots().await.map_err(|e| format!("{e:#}"))
    }
}
