use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use stellatune_core::TrackRef;

use super::{
    CommandCtx, DecodeCtrl, Event, PlayerState, StartSessionArgs, apply_dsp_chain,
    ensure_output_spec_prewarm, force_transition_gain_unity, handle_tick,
    maybe_fade_out_before_disrupt, output_backend_for_selected, resolve_output_spec_and_sink_chunk,
    set_state, start_session, stop_decode_session, sync_output_sink_with_active_session,
    track_ref_to_engine_token, track_ref_to_event_path,
};

pub(super) fn on_load_track(ctx: &mut CommandCtx<'_>, path: String) {
    maybe_fade_out_before_disrupt(ctx.state);
    stop_decode_session(ctx.state, ctx.track_info);
    ctx.state.current_track = Some(path.clone());
    ctx.state.position_ms = 0;
    ctx.state.wants_playback = false;
    ctx.state.pending_session_start = false;
    ctx.state.play_request_started_at = None;
    ctx.track_info.store(None);
    ctx.events.emit(Event::TrackChanged { path });
    ctx.events.emit(Event::Position {
        ms: ctx.state.position_ms,
    });
    set_state(ctx.state, ctx.events, PlayerState::Stopped);
}

pub(super) fn on_load_track_ref(ctx: &mut CommandCtx<'_>, track: TrackRef) {
    let Some(path) = track_ref_to_engine_token(&track) else {
        ctx.events.emit(Event::Error {
            message: "track locator is empty".to_string(),
        });
        return;
    };
    let Some(event_path) = track_ref_to_event_path(&track) else {
        ctx.events.emit(Event::Error {
            message: "track locator is empty".to_string(),
        });
        return;
    };
    maybe_fade_out_before_disrupt(ctx.state);
    stop_decode_session(ctx.state, ctx.track_info);
    ctx.state.current_track = Some(path.clone());
    ctx.state.position_ms = 0;
    ctx.state.wants_playback = false;
    ctx.state.pending_session_start = false;
    ctx.state.play_request_started_at = None;
    ctx.track_info.store(None);
    ctx.events.emit(Event::TrackChanged { path: event_path });
    ctx.events.emit(Event::Position {
        ms: ctx.state.position_ms,
    });
    set_state(ctx.state, ctx.events, PlayerState::Stopped);
}

pub(super) fn on_play(ctx: &mut CommandCtx<'_>) {
    let Some(path) = ctx.state.current_track.clone() else {
        ctx.events.emit(Event::Error {
            message: "no track loaded".to_string(),
        });
        return;
    };

    ctx.state.wants_playback = true;
    ctx.state.play_request_started_at = Some(Instant::now());

    if ctx.state.session.is_none() {
        set_state(ctx.state, ctx.events, PlayerState::Buffering);
        if let Some(cached_out_spec) = ctx.state.cached_output_spec {
            let out_spec = match resolve_output_spec_and_sink_chunk(ctx.state, cached_out_spec) {
                Ok(spec) => spec,
                Err(message) => {
                    ctx.events.emit(Event::Error { message });
                    set_state(ctx.state, ctx.events, PlayerState::Stopped);
                    ctx.state.wants_playback = false;
                    ctx.state.pending_session_start = false;
                    ctx.state.play_request_started_at = None;
                    return;
                }
            };
            let start_at_ms = ctx.state.position_ms.max(0) as u64;
            let Some(decode_worker) = ctx.state.decode_worker.as_ref() else {
                ctx.events.emit(Event::Error {
                    message: "decode worker unavailable".to_string(),
                });
                set_state(ctx.state, ctx.events, PlayerState::Stopped);
                ctx.state.wants_playback = false;
                ctx.state.pending_session_start = false;
                ctx.state.play_request_started_at = None;
                return;
            };
            let backend = output_backend_for_selected(ctx.state.selected_backend);
            match start_session(StartSessionArgs {
                path,
                decode_worker,
                internal_tx: ctx.internal_tx.clone(),
                backend,
                device_id: ctx.state.selected_device_id.clone(),
                match_track_sample_rate: ctx.state.match_track_sample_rate,
                gapless_playback: ctx.state.gapless_playback,
                out_spec,
                start_at_ms: start_at_ms as i64,
                volume: Arc::clone(&ctx.state.volume_atomic),
                lfe_mode: ctx.state.lfe_mode,
                output_sink_chunk_frames: ctx.state.output_sink_chunk_frames,
                output_sink_only: ctx.state.desired_output_sink_route.is_some(),
                output_pipeline: &mut ctx.state.output_pipeline,
            }) {
                Ok(session) => {
                    ctx.track_info
                        .store(Some(Arc::new(session.track_info.clone())));
                    ctx.state.session = Some(session);
                    if let Err(message) =
                        sync_output_sink_with_active_session(ctx.state, ctx.internal_tx)
                    {
                        ctx.events.emit(Event::Error { message });
                    }
                    if let Err(message) = apply_dsp_chain(ctx.state) {
                        ctx.events.emit(Event::Error { message });
                    }
                }
                Err(message) => {
                    ctx.events.emit(Event::Error { message });
                    set_state(ctx.state, ctx.events, PlayerState::Stopped);
                    ctx.state.wants_playback = false;
                    ctx.state.pending_session_start = false;
                    ctx.state.play_request_started_at = None;
                    return;
                }
            }
        } else {
            ctx.state.pending_session_start = true;
            ensure_output_spec_prewarm(ctx.state, ctx.internal_tx);
            return;
        }
    }

    if let Some(session) = ctx.state.session.as_ref() {
        if ctx.state.seek_track_fade {
            session
                .transition_gain
                .store(0.0f32.to_bits(), Ordering::Relaxed);
            session
                .transition_target_gain
                .store(0.0f32.to_bits(), Ordering::Relaxed);
        } else {
            force_transition_gain_unity(Some(session));
        }
        session.output_enabled.store(false, Ordering::Release);
        let _ = session.ctrl_tx.send(DecodeCtrl::Play);
    }

    set_state(ctx.state, ctx.events, PlayerState::Buffering);
    handle_tick(
        ctx.state,
        ctx.events,
        ctx.plugin_events,
        ctx.internal_tx,
        ctx.track_info,
    );
}

pub(super) fn on_pause(ctx: &mut CommandCtx<'_>) {
    if let Some(session) = ctx.state.session.as_ref() {
        maybe_fade_out_before_disrupt(ctx.state);
        session.output_enabled.store(false, Ordering::Release);
        let _ = session.ctrl_tx.send(DecodeCtrl::Pause);
    }
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
    ctx.state.pending_session_start = false;
    set_state(ctx.state, ctx.events, PlayerState::Paused);
}

pub(super) fn on_seek_ms(ctx: &mut CommandCtx<'_>, position_ms: u64) {
    let Some(_path) = ctx.state.current_track.clone() else {
        ctx.events.emit(Event::Error {
            message: "no track loaded".to_string(),
        });
        return;
    };

    maybe_fade_out_before_disrupt(ctx.state);
    ctx.state.position_ms = (position_ms as i64).max(0);
    ctx.events.emit(Event::Position {
        ms: ctx.state.position_ms,
    });

    if let Some(session) = ctx.state.session.as_ref() {
        session.output_enabled.store(false, Ordering::Release);
        let _ = session.ctrl_tx.send(DecodeCtrl::SeekMs {
            position_ms: ctx.state.position_ms,
        });
    }

    if ctx.state.wants_playback
        && matches!(
            ctx.state.player_state,
            PlayerState::Playing | PlayerState::Buffering
        )
    {
        set_state(ctx.state, ctx.events, PlayerState::Buffering);
        ctx.state.play_request_started_at = Some(Instant::now());
        handle_tick(
            ctx.state,
            ctx.events,
            ctx.plugin_events,
            ctx.internal_tx,
            ctx.track_info,
        );
    }
}

pub(super) fn on_stop(ctx: &mut CommandCtx<'_>) {
    stop_decode_session(ctx.state, ctx.track_info);
    ctx.state.position_ms = 0;
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
    ctx.state.pending_session_start = false;
    ctx.events.emit(Event::Position {
        ms: ctx.state.position_ms,
    });
    set_state(ctx.state, ctx.events, PlayerState::Stopped);
}
