use std::collections::HashMap;

use super::{ActorContext, LibraryServiceActor};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<HashMap<i64, crate::TrackLite>, String>)]
pub(crate) struct GetTracksMessage {
    pub(crate) track_ids: Vec<i64>,
}

impl Responder<GetTracksMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: GetTracksMessage,
        reply_to: ReplyTo<Result<HashMap<i64, crate::TrackLite>, String>>,
    ) -> Result<(), ActorError> {
        let result = self
            .worker
            .tracks_by_ids(&message.track_ids)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = reply_to.send(result);
        Ok(())
    }
}
