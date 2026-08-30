use super::{ActorContext, LibraryServiceActor};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<Vec<String>, String>)]
pub(crate) struct ListRootsMessage;

impl Responder<ListRootsMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        _message: ListRootsMessage,
        reply_to: ReplyTo<Result<Vec<String>, String>>,
    ) -> Result<(), ActorError> {
        let result = async { self.worker.list_roots().await.map_err(|e| format!("{e:#}")) }.await;
        let _ = reply_to.send(result);
        Ok(())
    }
}
