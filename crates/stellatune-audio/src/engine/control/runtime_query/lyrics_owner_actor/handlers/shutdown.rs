use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::runtime_query::lyrics_owner_actor::LyricsOwnerActor;

pub(crate) struct LyricsShutdownMessage;

impl Message for LyricsShutdownMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<LyricsShutdownMessage> for LyricsOwnerActor {
    async fn handle(
        &mut self,
        _message: LyricsShutdownMessage,
        ctx: &mut ActorContext<Self>,
    ) -> () {
        self.entry = None;
        ctx.stop();
    }
}
