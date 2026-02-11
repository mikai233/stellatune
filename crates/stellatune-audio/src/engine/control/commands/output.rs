use std::sync::atomic::Ordering;

use super::{
    CommandCtx, DecodeCtrl, Event, PlayerState, SessionStopMode, drop_output_pipeline,
    ensure_output_spec_prewarm, force_transition_gain_unity, output_backend_for_selected,
    parse_output_sink_route, set_state, stop_decode_session, sync_output_sink_with_active_session,
    ui_volume_to_gain,
};

pub(super) fn on_set_volume(ctx: &mut CommandCtx<'_>, volume: f32) {
    let ui = volume.clamp(0.0, 1.0);
    let gain = ui_volume_to_gain(ui);
    ctx.state.volume = ui;
    ctx.state
        .volume_atomic
        .store(gain.to_bits(), Ordering::Relaxed);
    ctx.events.emit(Event::VolumeChanged { volume: ui });
}

pub(super) fn on_set_lfe_mode(ctx: &mut CommandCtx<'_>, mode: stellatune_core::LfeMode) {
    ctx.state.lfe_mode = mode;
    if let Some(session) = ctx.state.session.as_ref() {
        let _ = session.ctrl_tx.send(DecodeCtrl::SetLfeMode { mode });
    }
}

pub(super) fn on_set_output_device(
    ctx: &mut CommandCtx<'_>,
    backend: stellatune_core::AudioBackend,
    device_id: Option<String>,
) {
    ctx.state.selected_backend = backend;
    ctx.state.selected_device_id = device_id;
    ctx.state.cached_output_spec = None;
    ctx.state.output_sink_negotiation_cache = None;
    ctx.state.output_spec_prewarm_inflight = false;
    ctx.state.output_spec_token = ctx.state.output_spec_token.wrapping_add(1);
    ensure_output_spec_prewarm(ctx.state, ctx.internal_tx);
    if ctx.state.session.is_some() {
        stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::TearDownSink);
    }
    drop_output_pipeline(ctx.state);
    if ctx.state.wants_playback {
        ctx.state.pending_session_start = true;
    }
}

pub(super) fn on_set_output_options(
    ctx: &mut CommandCtx<'_>,
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
) {
    if !seek_track_fade {
        force_transition_gain_unity(ctx.state.session.as_ref());
    }
    ctx.state.seek_track_fade = seek_track_fade;

    let changed = ctx.state.match_track_sample_rate != match_track_sample_rate
        || ctx.state.gapless_playback != gapless_playback;
    if changed {
        ctx.state.match_track_sample_rate = match_track_sample_rate;
        ctx.state.gapless_playback = gapless_playback;
        if ctx.state.session.is_some() {
            stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::TearDownSink);
            if ctx.state.wants_playback {
                ctx.state.pending_session_start = true;
            }
        }
        if !ctx.state.gapless_playback {
            drop_output_pipeline(ctx.state);
        }
    }
}

pub(super) fn on_set_output_sink_route(
    ctx: &mut CommandCtx<'_>,
    route: stellatune_core::OutputSinkRoute,
) {
    let parsed_route = match parse_output_sink_route(route) {
        Ok(route) => route,
        Err(message) => {
            ctx.events.emit(Event::Error { message });
            return;
        }
    };
    let mode_changed = ctx.state.desired_output_sink_route.is_none();
    let route_changed = ctx.state.desired_output_sink_route.as_ref() != Some(&parsed_route);
    ctx.state.desired_output_sink_route = Some(parsed_route);
    if mode_changed || route_changed {
        ctx.state.output_sink_chunk_frames = 0;
        ctx.state.output_sink_negotiation_cache = None;
        ctx.state.cached_output_spec = None;
        ctx.state.output_spec_prewarm_inflight = false;
        ctx.state.output_spec_token = ctx.state.output_spec_token.wrapping_add(1);
        ensure_output_spec_prewarm(ctx.state, ctx.internal_tx);
        let resume_playback = ctx.state.wants_playback;
        if ctx.state.session.is_some() {
            stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::TearDownSink);
            drop_output_pipeline(ctx.state);
        }
        if resume_playback {
            ctx.state.pending_session_start = true;
            set_state(ctx.state, ctx.events, PlayerState::Buffering);
        }
    }
    if let Err(message) = sync_output_sink_with_active_session(ctx.state, ctx.internal_tx) {
        ctx.events.emit(Event::Error { message });
    }
}

pub(super) fn on_clear_output_sink_route(ctx: &mut CommandCtx<'_>) {
    let mode_changed = ctx.state.desired_output_sink_route.is_some();
    ctx.state.desired_output_sink_route = None;
    ctx.state.output_sink_chunk_frames = 0;
    ctx.state.output_sink_negotiation_cache = None;
    if mode_changed {
        ctx.state.cached_output_spec = None;
        ctx.state.output_spec_prewarm_inflight = false;
        ctx.state.output_spec_token = ctx.state.output_spec_token.wrapping_add(1);
        ensure_output_spec_prewarm(ctx.state, ctx.internal_tx);
        let resume_playback = ctx.state.wants_playback;
        if ctx.state.session.is_some() {
            stop_decode_session(ctx.state, ctx.track_info, SessionStopMode::TearDownSink);
            drop_output_pipeline(ctx.state);
        }
        if resume_playback {
            ctx.state.pending_session_start = true;
            set_state(ctx.state, ctx.events, PlayerState::Buffering);
        }
    }
    if let Err(message) = sync_output_sink_with_active_session(ctx.state, ctx.internal_tx) {
        ctx.events.emit(Event::Error { message });
    }
}

pub(super) fn on_refresh_devices(ctx: &mut CommandCtx<'_>) {
    let selected_backend = output_backend_for_selected(ctx.state.selected_backend);
    let devices = stellatune_output::list_host_devices(Some(selected_backend))
        .into_iter()
        .map(|d| stellatune_core::AudioDevice {
            backend: match d.backend {
                stellatune_output::AudioBackend::Shared => stellatune_core::AudioBackend::Shared,
                stellatune_output::AudioBackend::WasapiExclusive => {
                    stellatune_core::AudioBackend::WasapiExclusive
                }
            },
            id: d.id,
            name: d.name,
        })
        .collect();
    ctx.events.emit(Event::OutputDevicesChanged { devices });
}
