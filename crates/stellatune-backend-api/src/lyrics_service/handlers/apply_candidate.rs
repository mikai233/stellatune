use anyhow::Result;
use stellatune_core::LyricsDoc;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;

pub(crate) struct ApplyCandidateMessage {
    pub(crate) track_key: String,
    pub(crate) doc: LyricsDoc,
}

impl Message for ApplyCandidateMessage {
    type Response = Result<()>;
}

#[async_trait::async_trait]
impl Handler<ApplyCandidateMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        message: ApplyCandidateMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<()> {
        self.core
            .apply_candidate(message.track_key, message.doc)
            .await
    }
}
