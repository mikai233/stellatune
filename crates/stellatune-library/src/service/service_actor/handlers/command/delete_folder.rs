use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct DeleteFolderMessage {
    pub(crate) path: String,
}

impl Handler<DeleteFolderMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: DeleteFolderMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self.worker.delete_folder(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
