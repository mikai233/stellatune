use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct RestoreFolderMessage {
    pub(crate) path: String,
}

impl Handler<RestoreFolderMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: RestoreFolderMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self.worker.restore_folder(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
