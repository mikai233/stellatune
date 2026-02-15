use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::engine::control::runtime_query::lyrics_owner_actor::LyricsOwnerActor;

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
