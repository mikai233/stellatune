use super::{CommandCtx, stop_all_audio};

pub(super) fn on_shutdown(ctx: &mut CommandCtx<'_>) -> bool {
    stop_all_audio(ctx.state, ctx.track_info);
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
    ctx.state.pending_session_start = false;
    true
}
