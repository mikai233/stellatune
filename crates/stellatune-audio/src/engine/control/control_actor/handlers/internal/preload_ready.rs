use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tracing::debug;

use crate::engine::control::control_actor::ControlActor;
use crate::engine::messages::PredecodedChunk;
use crate::engine::session::decode_worker::PromotedPreload;
use crate::types::TrackDecodeInfo;

use crate::engine::control::debug_metrics;

pub(crate) struct PreloadReadyInternalMessage {
    pub(crate) path: String,
    pub(crate) position_ms: u64,
    pub(crate) track_info: TrackDecodeInfo,
    pub(crate) chunk: PredecodedChunk,
    pub(crate) took_ms: u64,
    pub(crate) token: u64,
}

impl Message for PreloadReadyInternalMessage {
    type Response = ();
}

impl Handler<PreloadReadyInternalMessage> for ControlActor {
    fn handle(&mut self, message: PreloadReadyInternalMessage, _ctx: &mut ActorContext<Self>) {
        let PreloadReadyInternalMessage {
            path,
            position_ms,
            track_info,
            chunk,
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
        debug_metrics::note_preload_result(true, took_ms);
        if let Some(worker) = self.state.decode_worker.as_ref() {
            worker.promote_preload(PromotedPreload {
                path: path.clone(),
                position_ms,
                track_info,
                chunk,
            });
        }
        debug_metrics::maybe_log_preload_stats();
        debug!(%path, position_ms, took_ms, "preload cached");
    }
}
