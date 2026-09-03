use anyhow::Result;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::lyrics_service::actor::LyricsServiceActor;

#[derive(lattice_actor::Request)]
#[request(response = Result<()>)]
pub(crate) struct ClearCacheMessage;

impl Responder<ClearCacheMessage> for LyricsServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: ClearCacheMessage,
        reply_to: ReplyTo<Result<()>>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(self.core.clear_cache().await);
        Ok(())
    }
}
