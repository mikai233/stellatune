use super::{ActorContext, LibraryServiceActor};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<Vec<i64>, String>)]
pub(crate) struct ListLikedTrackIdsMessage;

impl Responder<ListLikedTrackIdsMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        _message: ListLikedTrackIdsMessage,
        reply_to: ReplyTo<Result<Vec<i64>, String>>,
    ) -> Result<(), ActorError> {
        let result = async {
            self.worker
                .list_liked_track_ids()
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;
        let _ = reply_to.send(result);
        Ok(())
    }
}
