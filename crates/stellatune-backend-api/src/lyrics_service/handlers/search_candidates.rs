use anyhow::Result;
use stellatune_core::{LyricsQuery, LyricsSearchCandidate};
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;

pub(in super::super) struct SearchCandidatesMessage {
    pub(in super::super) query: LyricsQuery,
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
