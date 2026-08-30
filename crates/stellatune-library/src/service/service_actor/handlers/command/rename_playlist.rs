use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct RenamePlaylistMessage {
    pub(crate) id: i64,
    pub(crate) name: String,
}

impl Handler<RenamePlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: RenamePlaylistMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self.worker.rename_playlist(message.id, message.name).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
