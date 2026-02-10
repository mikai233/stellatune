use std::sync::atomic::Ordering;
use std::time::Instant;

use tracing::trace;

use crate::engine::control::DisruptFadeKind;
use stellatune_core::TrackRef;

use super::{
    CommandCtx, DecodeCtrl, Event, PlayerState, SessionStopMode, ensure_output_spec_prewarm,
    force_transition_gain_unity, handle_tick, maybe_fade_out_before_disrupt, set_state,
    stop_decode_session, track_ref_to_engine_token, track_ref_to_event_path,
};

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

    let switch_id = ctx.state.switch_timing_seq;
    ctx.state.switch_timing_seq = ctx.state.switch_timing_seq.saturating_add(1);
    ctx.state.manual_switch_timing = Some(super::super::ManualSwitchTiming {
        id: switch_id,
        from_track: ctx.state.current_track.clone(),
        to_track: event_path.clone(),
        began_at: Instant::now(),
        fade_done_at: None,
        stop_done_at: None,
        committed_at: None,
        play_requested_at: None,
        session_ready_at: None,
    });

    let buffered_samples = ctx
        .state
        .session
        .as_ref()
        .map(|s| s.buffered_samples.load(Ordering::Relaxed))
        .unwrap_or(0);
    let sink_pending_samples = ctx
        .state
        .output_sink_worker
        .as_ref()
        .map(|w| w.pending_samples())
        .unwrap_or(0);
    trace!(
        switch_id,
        from_track = ctx.state.current_track.as_deref().unwrap_or("<none>"),
        to_track = event_path.as_str(),
        player_state = ?ctx.state.player_state,
        wants_playback = ctx.state.wants_playback,
        seek_track_fade = ctx.state.seek_track_fade,
        buffered_samples,
        sink_pending_samples,
        "manual track switch(ref) begin"
    );

    maybe_fade_out_before_disrupt(ctx.state, DisruptFadeKind::TrackSwitch);
    if let Some(timing) = ctx.state.manual_switch_timing.as_mut() {
        timing.fade_done_at = Some(Instant::now());
    }
    stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::KeepSink);
    if let Some(timing) = ctx.state.manual_switch_timing.as_mut() {
        timing.stop_done_at = Some(Instant::now());
    }

    ctx.state.current_track = Some(path.clone());
    super::super::next_position_session_id(ctx.state);
    ctx.state.position_ms = 0;
    ctx.state.wants_playback = false;
    ctx.state.pending_session_start = false;
    ctx.state.play_request_started_at = None;
    ctx.state.seek_position_guard = None;
    ctx.track_info.store(None);
    ctx.events.emit(Event::TrackChanged {
        path: event_path.clone(),
    });
    super::super::emit_position_event(ctx.state, ctx.events);
    set_state(ctx.state, ctx.events, PlayerState::Stopped);

    if let Some(timing) = ctx.state.manual_switch_timing.as_mut() {
        timing.committed_at = Some(Instant::now());
    }
    trace!(
        switch_id,
        track = event_path.as_str(),
        "manual track switch(ref) committed"
    );
}

pub(super) fn on_switch_track_ref(ctx: &mut CommandCtx<'_>, track: TrackRef, lazy: bool) {
    if track_ref_to_engine_token(&track).is_none() || track_ref_to_event_path(&track).is_none() {
        ctx.events.emit(Event::Error {
            message: "track locator is empty".to_string(),
        });
        return;
    }

    on_load_track_ref(ctx, track);
    if !lazy {
        let requested_at = Instant::now();
        if let Some(path) = ctx.state.current_track.as_deref()
            && let Some(timing) = ctx.state.manual_switch_timing.as_mut()
            && timing.to_track == path
            && timing.play_requested_at.is_none()
        {
            timing.play_requested_at = Some(requested_at);
        }
        ctx.state.wants_playback = true;
        ctx.state.play_request_started_at = Some(requested_at);
        ctx.state.pending_session_start = true;
        set_state(ctx.state, ctx.events, PlayerState::Buffering);
        ensure_output_spec_prewarm(ctx.state, ctx.internal_tx);
        handle_tick(
            ctx.state,
            ctx.events,
            ctx.plugin_events,
            ctx.internal_tx,
            ctx.track_info,
        );
    }
}

pub(super) fn on_play(ctx: &mut CommandCtx<'_>) {
    let Some(path) = ctx.state.current_track.clone() else {
        ctx.events.emit(Event::Error {
            message: "no track loaded".to_string(),
        });
        return;
    };

    let requested_at = Instant::now();
    ctx.state.wants_playback = true;
    ctx.state.play_request_started_at = Some(requested_at);
    if let Some(timing) = ctx.state.manual_switch_timing.as_mut()
        && timing.to_track == path
        && timing.play_requested_at.is_none()
    {
        timing.play_requested_at = Some(requested_at);
    }

    if ctx.state.session.is_none() {
        ctx.state.pending_session_start = true;
        set_state(ctx.state, ctx.events, PlayerState::Buffering);
        ensure_output_spec_prewarm(ctx.state, ctx.internal_tx);
        handle_tick(
            ctx.state,
            ctx.events,
            ctx.plugin_events,
            ctx.internal_tx,
            ctx.track_info,
        );
        return;
    }

    if let Some(session) = ctx.state.session.as_ref() {
        force_transition_gain_unity(Some(session));
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
        maybe_fade_out_before_disrupt(ctx.state, DisruptFadeKind::TrackSwitch);
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

    maybe_fade_out_before_disrupt(ctx.state, DisruptFadeKind::Seek);
    let target_ms = (position_ms as i64).max(0);
    let origin_ms = ctx.state.position_ms.max(0);
    ctx.state.seek_position_guard = Some(super::super::SeekPositionGuard {
        target_ms,
        origin_ms,
        requested_at: Instant::now(),
    });
    super::super::next_position_session_id(ctx.state);
    ctx.state.position_ms = target_ms;
    super::super::emit_position_event(ctx.state, ctx.events);

    if let Some(session) = ctx.state.session.as_ref() {
        session.output_enabled.store(false, Ordering::Release);
        let _ = session.ctrl_tx.send(DecodeCtrl::SeekMs {
            position_ms: target_ms,
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
    stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::TearDownSink);
    ctx.state.position_ms = 0;
    ctx.state.wants_playback = false;
    ctx.state.play_request_started_at = None;
    ctx.state.pending_session_start = false;
    ctx.state.seek_position_guard = None;
    super::super::next_position_session_id(ctx.state);
    super::super::emit_position_event(ctx.state, ctx.events);
    set_state(ctx.state, ctx.events, PlayerState::Stopped);
}
