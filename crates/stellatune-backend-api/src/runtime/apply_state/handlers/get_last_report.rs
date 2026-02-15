use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::runtime::apply_state::{ApplyStateCoordinatorActor, ApplyStateReport};

pub(crate) struct GetLastReportMessage;

impl Message for GetLastReportMessage {
    type Response = ApplyStateReport;
}

#[async_trait::async_trait]
impl Handler<GetLastReportMessage> for ApplyStateCoordinatorActor {
    async fn handle(
        &mut self,
        _message: GetLastReportMessage,
        _ctx: &mut ActorContext<Self>,
    ) -> ApplyStateReport {
        self.snapshot.last_report.clone()
    }
}
