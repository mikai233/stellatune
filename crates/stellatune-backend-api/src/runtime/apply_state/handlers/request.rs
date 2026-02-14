use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::ApplyStateCoordinatorActor;

pub(crate) struct RegisterRequestMessage;

impl Message for RegisterRequestMessage {
    type Response = u64;
}

#[async_trait::async_trait]
impl Handler<RegisterRequestMessage> for ApplyStateCoordinatorActor {
    async fn handle(
        &mut self,
        _message: RegisterRequestMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.snapshot.latest_requested_request_id =
            self.snapshot.latest_requested_request_id.max(request_id);
        request_id
    }
}
