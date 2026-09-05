use std::collections::HashMap;

use super::{ActorContext, LibraryServiceActor};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<HashMap<i64, String>, String>)]
pub(crate) struct GetTrackPathsMessage {
    pub(crate) track_ids: Vec<i64>,
}

impl Responder<GetTrackPathsMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: GetTrackPathsMessage,
        reply_to: ReplyTo<Result<HashMap<i64, String>, String>>,
    ) -> Result<(), ActorError> {
        let result = self
            .worker
            .track_paths(&message.track_ids)
            .await
            .map_err(|error| format!("{error:#}"));
        let _ = reply_to.send(result);
        Ok(())
    }
}
