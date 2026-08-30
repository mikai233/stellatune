use anyhow::Result;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::lyrics_service::LyricsServiceActor;

#[derive(lattice_actor::Request)]
#[request(response = Result<()>)]
pub(crate) struct RefreshCurrentMessage;

impl Responder<RefreshCurrentMessage> for LyricsServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: RefreshCurrentMessage,
        reply_to: ReplyTo<Result<()>>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(self.core.refresh_current().await);
        Ok(())
    }
}
