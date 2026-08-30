use anyhow::Result;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::{LyricsQuery, lyrics_service::LyricsServiceActor};

#[derive(lattice_actor::Request)]
#[request(response = Result<()>)]
pub(crate) struct PrefetchMessage {
    pub(crate) query: LyricsQuery,
}

impl Responder<PrefetchMessage> for LyricsServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: PrefetchMessage,
        reply_to: ReplyTo<Result<()>>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(self.core.prefetch(message.query).await);
        Ok(())
    }
}
