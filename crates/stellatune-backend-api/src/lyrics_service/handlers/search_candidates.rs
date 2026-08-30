use anyhow::Result;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};

use crate::{LyricsQuery, LyricsSearchCandidate, lyrics_service::LyricsServiceActor};

#[derive(lattice_actor::Request)]
#[request(response = Result<Vec<LyricsSearchCandidate>>)]
pub(crate) struct SearchCandidatesMessage {
    pub(crate) query: LyricsQuery,
}

impl Responder<SearchCandidatesMessage> for LyricsServiceActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: SearchCandidatesMessage,
        reply_to: ReplyTo<Result<Vec<LyricsSearchCandidate>>>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(self.core.search_candidates(message.query).await);
        Ok(())
    }
}
