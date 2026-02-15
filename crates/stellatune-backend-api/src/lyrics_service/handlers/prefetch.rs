use anyhow::Result;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;
use crate::LyricsQuery;

pub(crate) struct PrefetchMessage {
    pub(crate) query: LyricsQuery,
}

impl Message for PrefetchMessage {
    type Response = Result<()>;
}

#[async_trait::async_trait]
impl Handler<PrefetchMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        message: PrefetchMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<()> {
        self.core.prefetch(message.query).await
    }
}
