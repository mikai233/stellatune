use super::{ActorContext, LibraryServiceActor, TrackLite};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<Vec<TrackLite>, String>)]
pub(crate) struct SearchTracksMessage {
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Responder<SearchTracksMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: SearchTracksMessage,
        reply_to: ReplyTo<Result<Vec<TrackLite>, String>>,
    ) -> Result<(), ActorError> {
        let result = async {
            self.worker
                .search(message.query, message.limit, message.offset)
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;
        let _ = reply_to.send(result);
        Ok(())
    }
}
