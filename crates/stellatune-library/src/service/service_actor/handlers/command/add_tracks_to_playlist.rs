use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct AddTracksToPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_ids: Vec<i64>,
}

impl Handler<AddTracksToPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: AddTracksToPlaylistMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self
            .worker
            .add_tracks_to_playlist(message.playlist_id, message.track_ids)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
