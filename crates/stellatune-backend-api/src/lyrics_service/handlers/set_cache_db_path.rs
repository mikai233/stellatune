use anyhow::Result;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::lyrics_service::LyricsServiceActor;

pub(crate) struct SetCacheDbPathMessage {
    pub(crate) db_path: String,
}

impl Message for SetCacheDbPathMessage {
    type Response = Result<()>;
}

#[async_trait::async_trait]
impl Handler<SetCacheDbPathMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        message: SetCacheDbPathMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<()> {
        self.core.set_cache_db_path(message.db_path).await
    }
}
