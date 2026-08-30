use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct RemoveRootMessage {
    pub(crate) path: String,
}

impl Handler<RemoveRootMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: RemoveRootMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self.worker.remove_root(message.path).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
