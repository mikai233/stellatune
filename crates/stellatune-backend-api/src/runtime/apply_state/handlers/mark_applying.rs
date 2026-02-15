use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::runtime::apply_state::{ApplyStateCoordinatorActor, now_ms_for_actor};

pub(crate) struct MarkApplyingMessage {
    pub(crate) request_id: u64,
}

impl Message for MarkApplyingMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<MarkApplyingMessage> for ApplyStateCoordinatorActor {
    async fn handle(&mut self, message: MarkApplyingMessage, _ctx: &mut ActorContext<Self>) -> () {
        self.snapshot.phase = "applying";
        self.snapshot.request_id = message.request_id;
        self.snapshot.last_started_at_ms = now_ms_for_actor();
        self.snapshot.last_finished_at_ms = 0;
    }
}
