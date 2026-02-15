use anyhow::Result;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;
use crate::{LyricsQuery, LyricsSearchCandidate};

pub(crate) struct SearchCandidatesMessage {
    pub(crate) query: LyricsQuery,
}

impl Message for SearchCandidatesMessage {
    type Response = Result<Vec<LyricsSearchCandidate>>;
}

#[async_trait::async_trait]
impl Handler<SearchCandidatesMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        message: SearchCandidatesMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<Vec<LyricsSearchCandidate>> {
        self.core.search_candidates(message.query).await
    }
}
