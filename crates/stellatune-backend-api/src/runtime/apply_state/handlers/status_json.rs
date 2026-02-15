use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::runtime::apply_state::{ApplyStateCoordinatorActor, snapshot_status_json};

pub(crate) struct GetStatusJsonMessage;

impl Message for GetStatusJsonMessage {
    type Response = String;
}

#[async_trait::async_trait]
impl Handler<GetStatusJsonMessage> for ApplyStateCoordinatorActor {
    async fn handle(
        &mut self,
        _message: GetStatusJsonMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> String {
        snapshot_status_json(&self.snapshot)
    }
}
