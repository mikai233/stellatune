use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tracing::debug;

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::debug_metrics;

pub(crate) struct PreloadFailedInternalMessage {
    pub(crate) path: String,
    pub(crate) position_ms: u64,
    pub(crate) message: String,
    pub(crate) took_ms: u64,
    pub(crate) token: u64,
}

impl Message for PreloadFailedInternalMessage {
    type Response = ();
}

impl Handler<PreloadFailedInternalMessage> for ControlActor {
    fn handle(&mut self, message: PreloadFailedInternalMessage, _ctx: &mut ActorContext<Self>) {
        let PreloadFailedInternalMessage {
            path,
            position_ms,
            message,
            took_ms,
            token,
        } = message;
        if token != self.state.preload_token {
            return;
        }
        if self.state.requested_preload_path.as_deref() != Some(path.as_str()) {
            return;
        }
        if self.state.requested_preload_position_ms != position_ms {
            return;
        }
        debug_metrics::note_preload_result(false, took_ms);
        debug_metrics::maybe_log_preload_stats();
        debug!(%path, position_ms, took_ms, "preload failed: {message}");
    }
}
