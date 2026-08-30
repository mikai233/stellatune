use super::{ActorContext, LibraryServiceActor, TrackLite};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<Vec<TrackLite>, String>)]
pub(crate) struct ListTracksMessage {
    pub(crate) folder: String,
    pub(crate) recursive: bool,
    pub(crate) query: String,
    pub(crate) limit: i64,
    pub(crate) offset: i64,
}

impl Responder<ListTracksMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        message: ListTracksMessage,
        reply_to: ReplyTo<Result<Vec<TrackLite>, String>>,
    ) -> Result<(), ActorError> {
        let result = async {
            self.worker
                .list_tracks(
                    message.folder,
                    message.recursive,
                    message.query,
                    message.limit,
                    message.offset,
                )
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;
        let _ = reply_to.send(result);
        Ok(())
    }
}
