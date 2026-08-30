use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct CreatePlaylistMessage {
    pub(crate) name: String,
}

impl Handler<CreatePlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: CreatePlaylistMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self.worker.create_playlist(message.name).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
