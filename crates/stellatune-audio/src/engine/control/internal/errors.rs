use std::time::Instant;

use tracing::error;

use super::{
    Event, InternalCtx, PlayerState, drop_output_pipeline, ensure_output_spec_prewarm,
    event_path_from_engine_token, set_state, stop_all_audio, stop_decode_session,
};

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
    stop_decode_session(ctx.state, ctx.track_info);
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
    set_state(ctx.state, ctx.events, PlayerState::Stopped);
}

pub(super) fn on_error(ctx: &mut InternalCtx<'_>, message: String) {
    ctx.events.emit(Event::Error { message });
    stop_decode_session(ctx.state, ctx.track_info);
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
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
    stop_decode_session(ctx.state, ctx.track_info);
    drop_output_pipeline(ctx.state);

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

pub(super) fn on_position(ctx: &mut InternalCtx<'_>, ms: i64) {
    ctx.state.position_ms = ms;
}
