use anyhow::Result;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::lyrics_service::LyricsServiceActor;

#[derive(lattice_actor::Request)]
#[request(response = Result<()>)]
pub(crate) struct SetCacheDbPathMessage {
    pub(crate) db_path: String,
}

impl Responder<SetCacheDbPathMessage> for LyricsServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: SetCacheDbPathMessage,
        reply_to: ReplyTo<Result<()>>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(self.core.set_cache_db_path(message.db_path).await);
        Ok(())
    }
}
