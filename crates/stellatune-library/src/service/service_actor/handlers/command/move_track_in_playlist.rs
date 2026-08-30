use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct MoveTrackInPlaylistMessage {
    pub(crate) playlist_id: i64,
    pub(crate) track_id: i64,
    pub(crate) new_index: i64,
}

impl Handler<MoveTrackInPlaylistMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: MoveTrackInPlaylistMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self
            .worker
            .move_track_in_playlist(message.playlist_id, message.track_id, message.new_index)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
