use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct DeletePlaylistMessage {
    pub(crate) id: i64,
}

impl Handler<DeletePlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: DeletePlaylistMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self.worker.delete_playlist(message.id).await {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
