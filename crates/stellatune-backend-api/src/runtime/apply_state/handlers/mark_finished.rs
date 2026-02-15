use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use crate::runtime::apply_state::{
    ApplyStateCoordinatorActor, ApplyStateReport, ApplyStateRunResult, now_ms_for_actor,
};

pub(crate) struct MarkFinishedMessage {
    pub(crate) request_id: u64,
    pub(crate) result: ApplyStateRunResult,
}

impl Message for MarkFinishedMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<MarkFinishedMessage> for ApplyStateCoordinatorActor {
    async fn handle(&mut self, message: MarkFinishedMessage, _ctx: &mut ActorContext<Self>) -> () {
        self.snapshot.request_id = message.request_id;
        self.snapshot.last_completed_request_id = message.request_id;
        self.snapshot.last_finished_at_ms = now_ms_for_actor();
        match message.result {
            ApplyStateRunResult::Success(report) => {
                self.snapshot.phase = if report.errors.is_empty() {
                    "applied"
                } else {
                    "failed"
                };
                self.snapshot.last_report = report;
            },
            ApplyStateRunResult::Failure(error) => {
                self.snapshot.phase = "failed";
                let mut report = ApplyStateReport::empty_completed();
                report.phase = "failed";
                report.errors = vec![error];
                self.snapshot.last_report = report;
            },
        }
    }
}
