use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::ApplyStateCoordinatorActor;

pub(crate) struct GetLastCompletedRequestIdMessage;

impl Message for GetLastCompletedRequestIdMessage {
    type Response = u64;
}

#[async_trait::async_trait]
impl Handler<GetLastCompletedRequestIdMessage> for ApplyStateCoordinatorActor {
    async fn handle(
        &mut self,
        _message: GetLastCompletedRequestIdMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> u64 {
        self.snapshot.last_completed_request_id
    }
}
