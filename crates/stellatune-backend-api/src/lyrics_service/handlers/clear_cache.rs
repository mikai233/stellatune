use anyhow::Result;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;

pub(in super::super) struct ClearCacheMessage;

impl Message for ClearCacheMessage {
    type Response = Result<()>;
}

#[async_trait::async_trait]
impl Handler<ClearCacheMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        _message: ClearCacheMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<()> {
        self.core.clear_cache().await
    }
}
