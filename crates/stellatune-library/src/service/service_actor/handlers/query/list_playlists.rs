use super::{ActorContext, LibraryServiceActor, PlaylistLite};
use lattice_actor::{error::ActorError, reply::ReplyTo, traits::Responder};

#[derive(lattice_actor::Request)]
#[request(response = Result<Vec<PlaylistLite>, String>)]
pub(crate) struct ListPlaylistsMessage;

impl Responder<ListPlaylistsMessage> for LibraryServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut ActorContext<'_, Self>,
        _message: ListPlaylistsMessage,
        reply_to: ReplyTo<Result<Vec<PlaylistLite>, String>>,
    ) -> Result<(), ActorError> {
        let result = async {
            self.worker
                .list_playlists()
                .await
                .map_err(|e| format!("{e:#}"))
        }
        .await;
        let _ = reply_to.send(result);
        Ok(())
    }
}
