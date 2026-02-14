use anyhow::Result;
use stellatune_core::LyricsQuery;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;

pub(in super::super) struct PrepareMessage {
    pub(in super::super) query: LyricsQuery,
}

impl Message for PrepareMessage {
    type Response = Result<()>;
}

#[async_trait::async_trait]
impl Handler<PrepareMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        message: PrepareMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<()> {
        self.core.prepare(message.query).await
    }
}
