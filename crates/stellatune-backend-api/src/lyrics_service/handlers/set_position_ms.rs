use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::LyricsServiceActor;

pub(in super::super) struct SetPositionMsMessage {
    pub(in super::super) position_ms: u64,
}

impl Message for SetPositionMsMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<SetPositionMsMessage> for LyricsServiceActor {
    async fn handle(&mut self, message: SetPositionMsMessage, _ctx: &mut ActorContext<Self>) -> () {
        self.core.set_position_ms(message.position_ms);
    }
}
