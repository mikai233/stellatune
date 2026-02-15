use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::runtime::apply_state::ApplyStateCoordinatorActor;

pub(crate) struct GetLatestRequestedRequestIdMessage;

impl Message for GetLatestRequestedRequestIdMessage {
    type Response = u64;
}

#[async_trait::async_trait]
impl Handler<GetLatestRequestedRequestIdMessage> for ApplyStateCoordinatorActor {
    async fn handle(
        &mut self,
        _message: GetLatestRequestedRequestIdMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> u64 {
        self.snapshot.latest_requested_request_id
    }
}
