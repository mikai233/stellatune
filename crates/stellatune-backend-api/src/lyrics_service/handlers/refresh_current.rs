use anyhow::Result;
use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;

pub(in super::super) struct RefreshCurrentMessage;

impl Message for RefreshCurrentMessage {
    type Response = Result<()>;
}

#[async_trait::async_trait]
impl Handler<RefreshCurrentMessage> for LyricsServiceActor {
    async fn handle(
        &mut self,
        _message: RefreshCurrentMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> Result<()> {
        self.core.refresh_current().await
    }
}
