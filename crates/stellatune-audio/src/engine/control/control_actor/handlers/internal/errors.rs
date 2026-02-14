use std::time::Instant;

use tracing::error;

use super::super::super::super::{
    Event, PlayerState, SessionStopMode, drop_output_pipeline, ensure_output_spec_prewarm,
    event_path_from_engine_token, set_state, stop_all_audio, stop_decode_session,
};
use super::InternalCtx;

const SEEK_POSITION_GUARD_TIMEOUT_MS: u64 = 1200;
const SEEK_POSITION_ACCEPT_TOLERANCE_MS: i64 = 400;

pub(super) fn on_eof(ctx: &mut InternalCtx<'_>) {
    ctx.events.emit(Event::Log {
        message: "end of stream".to_string(),
    });
    if ctx.state.wants_playback
        && let Some(path) = ctx.state.current_track.clone()
    {
        ctx.events.emit(Event::PlaybackEnded {
            path: event_path_from_engine_token(&path),
        });
    }
    stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::KeepSink);
    ctx.state.seek_position_guard = None;
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
    ctx.state.pending_session_start = false;
    set_state(ctx.state, ctx.events, PlayerState::Stopped);
}

pub(super) fn on_error(ctx: &mut InternalCtx<'_>, message: String) {
    ctx.events.emit(Event::Error { message });
    stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::TearDownSink);
    ctx.state.seek_position_guard = None;
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
    ctx.state.pending_session_start = false;
    set_state(ctx.state, ctx.events, PlayerState::Stopped);
}

pub(super) fn on_output_error(ctx: &mut InternalCtx<'_>, message: String) {
    if ctx.state.session.is_none() {
        error!("output stream error (no active session): {message}");
        ctx.events.emit(Event::Log {
            message: format!("output stream error (no active session): {message}"),
        });
        return;
    }

    error!("output stream error: {message}");
    ctx.events.emit(Event::Error {
        message: format!("output stream error: {message}"),
    });

    let Some(_path) = ctx.state.current_track.clone() else {
        stop_all_audio(ctx.state, ctx.track_info);
        ctx.state.wants_playback = false;
        set_state(ctx.state, ctx.events, PlayerState::Stopped);
        return;
    };

    let prev_state = ctx.state.player_state;
    stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::TearDownSink);
    drop_output_pipeline(ctx.state);
    ctx.state.seek_position_guard = None;

    ctx.state.cached_output_spec = None;
    ensure_output_spec_prewarm(ctx.state, ctx.internal_tx);
    ctx.state.pending_session_start =
        prev_state == PlayerState::Playing || prev_state == PlayerState::Buffering;

    match prev_state {
        PlayerState::Playing | PlayerState::Buffering => {
            ctx.state.wants_playback = true;
            ctx.state.play_request_started_at = Some(Instant::now());
            set_state(ctx.state, ctx.events, PlayerState::Buffering);
        }
        PlayerState::Paused => {
            ctx.state.wants_playback = false;
            ctx.state.play_request_started_at = None;
            set_state(ctx.state, ctx.events, PlayerState::Paused);
        }
        PlayerState::Stopped => {
            ctx.state.wants_playback = false;
            ctx.state.play_request_started_at = None;
            set_state(ctx.state, ctx.events, PlayerState::Stopped);
        }
    }

    ctx.events.emit(Event::Log {
        message: "output error: scheduled session restart".to_string(),
    });
}

pub(super) fn on_position(ctx: &mut InternalCtx<'_>, path: String, ms: i64) {
    if ctx.state.session.is_none() {
        return;
    }
    if ctx.state.current_track.as_deref() != Some(path.as_str()) {
        return;
    }
    if let Some(guard) = ctx.state.seek_position_guard.as_ref() {
        let elapsed_ms = guard.requested_at.elapsed().as_millis() as u64;
        let forward = guard.target_ms >= guard.origin_ms;
        let reached_target = if forward {
            ms >= guard
                .target_ms
                .saturating_sub(SEEK_POSITION_ACCEPT_TOLERANCE_MS)
        } else {
            ms <= guard
                .target_ms
                .saturating_add(SEEK_POSITION_ACCEPT_TOLERANCE_MS)
        };
        if reached_target || elapsed_ms >= SEEK_POSITION_GUARD_TIMEOUT_MS {
            ctx.state.seek_position_guard = None;
        } else {
            return;
        }
    }
    if ctx.state.player_state == PlayerState::Buffering
        && ctx.state.position_ms <= 1000
        && ms > 5000
    {
        return;
    }
    ctx.state.position_ms = ms;
    super::super::super::super::emit_position_event(ctx.state, ctx.events);
}
