use stellatune_core::TrackRef;

use super::{CommandCtx, debug_metrics, enqueue_preload_task, track_ref_to_engine_token};

pub(super) fn on_preload_track(
    ctx: &mut CommandCtx<'_>,
    path: String,
    position_ms: u64,
) -> Result<(), String> {
    let path = path.trim().to_string();
    if path.is_empty() {
        return Ok(());
    }
    if ctx.state.requested_preload_path.as_deref() == Some(path.as_str())
        && ctx.state.requested_preload_position_ms == position_ms
    {
        return Ok(());
    }
    ctx.state.requested_preload_path = Some(path.clone());
    ctx.state.requested_preload_position_ms = position_ms;
    ctx.state.preload_token = ctx.state.preload_token.wrapping_add(1);
    debug_metrics::note_preload_request();
    enqueue_preload_task(ctx.state, path, position_ms, ctx.state.preload_token);
    Ok(())
}

pub(super) fn on_preload_track_ref(
    ctx: &mut CommandCtx<'_>,
    track: TrackRef,
    position_ms: u64,
) -> Result<(), String> {
    let Some(path) = track_ref_to_engine_token(&track) else {
        return Err("track locator is empty".to_string());
    };
    if ctx.state.requested_preload_path.as_deref() == Some(path.as_str())
        && ctx.state.requested_preload_position_ms == position_ms
    {
        return Ok(());
    }
    ctx.state.requested_preload_path = Some(path.clone());
    ctx.state.requested_preload_position_ms = position_ms;
    ctx.state.preload_token = ctx.state.preload_token.wrapping_add(1);
    debug_metrics::note_preload_request();
    enqueue_preload_task(ctx.state, path, position_ms, ctx.state.preload_token);
    Ok(())
}
