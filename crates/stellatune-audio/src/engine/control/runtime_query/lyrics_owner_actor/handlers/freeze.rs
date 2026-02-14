use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsOwnerActor;

pub(crate) struct LyricsFreezeMessage;

impl Message for LyricsFreezeMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<LyricsFreezeMessage> for LyricsOwnerActor {
    async fn handle(&mut self, _message: LyricsFreezeMessage, _ctx: &mut ActorContext<Self>) -> () {
        self.frozen = true;
    }
}
