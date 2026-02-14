use std::time::Instant;

use stellatune_runtime::tokio_actor::{ActorContext, Handler, Message};

use super::super::RuntimeRouterActor;
use crate::runtime::bus::{ControlFinishedArgs, drain_timed_out_pending, emit_control_finished};

pub(crate) struct DrainTimeoutMessage;

impl Message for DrainTimeoutMessage {
    type Response = ();
}

#[async_trait::async_trait]
impl Handler<DrainTimeoutMessage> for RuntimeRouterActor {
    async fn handle(&mut self, _message: DrainTimeoutMessage, _ctx: &mut ActorContext<Self>) -> () {
        for timed_out in drain_timed_out_pending(&mut self.pending_finishes, Instant::now()) {
            emit_control_finished(
                self.router.runtime_hub.as_ref(),
                ControlFinishedArgs {
                    plugin_id: &timed_out.plugin_id,
                    request_id: timed_out.request_id,
                    scope: timed_out.scope,
                    command: timed_out.command,
                    error: Some("control finish timeout"),
                },
            )
            .await;
        }
    }
}
