use anyhow::Result;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::LyricsDoc;
use crate::lyrics_service::actor::LyricsServiceActor;

#[derive(lattice_actor::Request)]
#[request(response = Result<()>)]
pub(crate) struct ApplyCandidateMessage {
    pub(crate) track_key: String,
    pub(crate) doc: LyricsDoc,
}

impl Responder<ApplyCandidateMessage> for LyricsServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: ApplyCandidateMessage,
        reply_to: ReplyTo<Result<()>>,
    ) -> Result<(), ActorError> {
        let result = self
            .core
            .apply_candidate(message.track_key, message.doc)
            .await;
        let _ = reply_to.send(result);
        Ok(())
    }
}
