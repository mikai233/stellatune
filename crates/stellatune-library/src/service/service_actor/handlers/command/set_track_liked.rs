use super::{ActorContext, LibraryEvent, LibraryServiceActor};
use lattice_actor::{error::ActorError, traits::Handler};

#[derive(lattice_actor::Message)]
pub(crate) struct SetTrackLikedMessage {
    pub(crate) track_id: i64,
    pub(crate) liked: bool,
}

impl Handler<SetTrackLikedMessage> for LibraryServiceActor {
    async fn handle(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: SetTrackLikedMessage,
    ) -> Result<(), ActorError> {
        if let Err(err) = self
            .worker
            .set_track_liked(message.track_id, message.liked)
            .await
        {
            self.events.emit(LibraryEvent::Error {
                message: format!("{err:#}"),
            });
        }
        Ok(())
    }
}
