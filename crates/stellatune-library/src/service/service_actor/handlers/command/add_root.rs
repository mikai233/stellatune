use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct AddRootMessage {
    pub(crate) path: String,
}

impl Handler<AddRootMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: AddRootMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self.worker.add_root(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
