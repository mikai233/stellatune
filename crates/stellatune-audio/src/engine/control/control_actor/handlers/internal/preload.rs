use tracing::debug;

use stellatune_core::TrackDecodeInfo;

use crate::engine::messages::PredecodedChunk;
use crate::engine::session::PromotedPreload;

use super::super::super::super::debug_metrics;
use super::InternalCtx;

pub(super) struct PreloadReadyArgs {
    pub(super) path: String,
    pub(super) position_ms: u64,
    pub(super) track_info: TrackDecodeInfo,
    pub(super) chunk: PredecodedChunk,
    pub(super) took_ms: u64,
    pub(super) token: u64,
}

pub(super) fn on_preload_ready(ctx: &mut InternalCtx<'_>, args: PreloadReadyArgs) {
    let PreloadReadyArgs {
        path,
        position_ms,
        track_info,
        chunk,
        took_ms,
        token,
    } = args;

    if token != ctx.state.preload_token {
        return;
    }
    if ctx.state.requested_preload_path.as_deref() != Some(path.as_str()) {
        return;
    }
    if ctx.state.requested_preload_position_ms != position_ms {
        return;
    }
    debug_metrics::note_preload_result(true, took_ms);
    if let Some(worker) = ctx.state.decode_worker.as_ref() {
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

pub(super) fn on_preload_failed(
    ctx: &mut InternalCtx<'_>,
    path: String,
    position_ms: u64,
    message: String,
    took_ms: u64,
    token: u64,
) {
    if token != ctx.state.preload_token {
        return;
    }
    if ctx.state.requested_preload_path.as_deref() != Some(path.as_str()) {
        return;
    }
    if ctx.state.requested_preload_position_ms != position_ms {
        return;
    }
    debug_metrics::note_preload_result(false, took_ms);
    debug_metrics::maybe_log_preload_stats();
    debug!(%path, position_ms, took_ms, "preload failed: {message}");
}
