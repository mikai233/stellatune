use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct RemoveTracksFromPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_ids: Vec<i64>,
}

impl Handler<RemoveTracksFromPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: RemoveTracksFromPlaylistMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self
            .worker
            .remove_tracks_from_playlist(message.playlist_id, message.track_ids)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
