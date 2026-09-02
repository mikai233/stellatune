use super::{ActorContext, LibraryServiceActor, TrackLite};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<Option<TrackLite>, String>)]
pub(crate) struct GetTrackMessage {
    pub(crate) track_id: i64,
}

impl Responder<GetTrackMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: GetTrackMessage,
        reply_to: ReplyTo<Result<Option<TrackLite>, String>>,
    ) -> Result<(), ActorError> {
        let result = self
            .worker
            .track_by_id(message.track_id)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = reply_to.send(result);
        Ok(())
    }
}
