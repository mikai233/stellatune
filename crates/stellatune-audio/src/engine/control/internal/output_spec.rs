use tracing::{debug, warn};

use stellatune_output::OutputSpec;

use super::{Event, InternalCtx, PlayerState, set_state};

pub(super) fn on_output_spec_ready(
    ctx: &mut InternalCtx<'_>,
    spec: OutputSpec,
    took_ms: u64,
    token: u64,
) {
    if token != ctx.state.output_spec_token {
        return;
    }
    ctx.state.cached_output_spec = Some(spec);
    ctx.state.output_spec_prewarm_inflight = false;
    debug!(
        "output_spec prewarm ready in {}ms: {}Hz {}ch",
        took_ms, spec.sample_rate, spec.channels
    );
}

pub(super) fn on_output_spec_failed(
    ctx: &mut InternalCtx<'_>,
    message: String,
    took_ms: u64,
    token: u64,
) {
    if token != ctx.state.output_spec_token {
        return;
    }
    ctx.state.cached_output_spec = None;
    ctx.state.output_spec_prewarm_inflight = false;
    warn!("output_spec prewarm failed in {}ms: {}", took_ms, message);
    if ctx.state.wants_playback && ctx.state.session.is_none() {
        ctx.state.pending_session_start = false;
        ctx.state.wants_playback = false;
        ctx.state.play_request_started_at = None;
        ctx.events.emit(Event::Error {
            message: format!("failed to query output device: {message}"),
        });
        set_state(ctx.state, ctx.events, PlayerState::Stopped);
    }
}
