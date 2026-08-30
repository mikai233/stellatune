use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct AddTrackToPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_id: i64,
}

impl Handler<AddTrackToPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: AddTrackToPlaylistMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self
            .worker
            .add_track_to_playlist(message.playlist_id, message.track_id)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
