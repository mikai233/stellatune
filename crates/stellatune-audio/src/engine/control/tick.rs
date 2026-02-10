use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Instant;

use crossbeam_channel::Sender;
use tracing::debug;

use stellatune_core::{Event, HostEventTopic, HostPlayerTickPayload, PlayerState};
use stellatune_output::output_spec_for_device;

use super::{
    BUFFER_HIGH_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE, BUFFER_LOW_WATERMARK_MS,
    BUFFER_LOW_WATERMARK_MS_EXCLUSIVE, DecodeCtrl, EngineState, EventHub, InternalMsg,
    PluginEventHub, SharedTrackInfo, StartSessionArgs, UNDERRUN_LOG_INTERVAL, apply_dsp_chain,
    force_transition_gain_unity, output_sink_queue_watermarks_ms, output_spec_for_plugin_sink,
    resolve_output_spec_and_sink_chunk, set_state, start_session,
    sync_output_sink_with_active_session,
};

pub(super) fn ensure_output_spec_prewarm(
    state: &mut EngineState,
    internal_tx: &Sender<InternalMsg>,
) {
    if state.cached_output_spec.is_some() || state.output_spec_prewarm_inflight {
        return;
    }

    if state.desired_output_sink_route.is_some() {
        let spec = output_spec_for_plugin_sink(state);
        state.cached_output_spec = Some(spec);
        state.output_spec_prewarm_inflight = false;
        debug!(
            "output_spec prewarm bypassed for plugin sink: {}Hz {}ch",
            spec.sample_rate, spec.channels
        );
        return;
    }

    state.output_spec_prewarm_inflight = true;
    let token = state.output_spec_token;
    let backend = output_backend_for_selected(state.selected_backend);
    let device_id = state.selected_device_id.clone();
    let tx = internal_tx.clone();
    thread::Builder::new()
        .name("stellatune-output-spec".to_string())
        .spawn(move || {
            let t0 = Instant::now();
            match output_spec_for_device(backend, device_id) {
                Ok(spec) => {
                    let _ = tx.send(InternalMsg::OutputSpecReady {
                        spec,
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
                Err(e) => {
                    let _ = tx.send(InternalMsg::OutputSpecFailed {
                        message: e.to_string(),
                        took_ms: t0.elapsed().as_millis() as u64,
                        token,
                    });
                }
            }
        })
        .expect("failed to spawn stellatune-output-spec thread");
}

pub(super) fn output_backend_for_selected(
    backend: stellatune_core::AudioBackend,
) -> stellatune_output::AudioBackend {
    match backend {
        stellatune_core::AudioBackend::Shared => stellatune_output::AudioBackend::Shared,
        stellatune_core::AudioBackend::WasapiExclusive => {
            stellatune_output::AudioBackend::WasapiExclusive
        }
    }
}

pub(super) fn publish_player_tick_event(state: &EngineState) {
    let event_json = match serde_json::to_string(&HostPlayerTickPayload {
        topic: HostEventTopic::PlayerTick,
        state: state.player_state,
        position_ms: state.position_ms,
        track: state.current_track.clone(),
        wants_playback: state.wants_playback,
    }) {
        Ok(v) => v,
        Err(_) => return,
    };

    stellatune_plugins::broadcast_shared_host_event_json(&event_json);
}

pub(super) fn handle_tick(
    state: &mut EngineState,
    events: &Arc<EventHub>,
    _plugin_events: &Arc<PluginEventHub>,
    internal_tx: &Sender<InternalMsg>,
    track_info: &SharedTrackInfo,
) {
    // If we are waiting for an output spec (prewarm) and have no active session, start the session
    // as soon as the spec becomes available.
    if state.session.is_none()
        && state.wants_playback
        && state.pending_session_start
        && state.cached_output_spec.is_some()
    {
        let Some(path) = state.current_track.clone() else {
            state.pending_session_start = false;
            state.wants_playback = false;
            state.play_request_started_at = None;
            set_state(state, events, PlayerState::Stopped);
            return;
        };
        let Some(cached_out_spec) = state.cached_output_spec else {
            state.pending_session_start = false;
            state.wants_playback = false;
            state.play_request_started_at = None;
            events.emit(Event::Error {
                message: "output spec missing while pending session start".to_string(),
            });
            set_state(state, events, PlayerState::Stopped);
            return;
        };
        let out_spec = match resolve_output_spec_and_sink_chunk(state, cached_out_spec) {
            Ok(spec) => spec,
            Err(message) => {
                state.pending_session_start = false;
                state.wants_playback = false;
                state.play_request_started_at = None;
                events.emit(Event::Error { message });
                set_state(state, events, PlayerState::Stopped);
                return;
            }
        };
        let start_at_ms = state.position_ms.max(0) as u64;
        let Some(decode_worker) = state.decode_worker.as_ref() else {
            state.pending_session_start = false;
            state.wants_playback = false;
            state.play_request_started_at = None;
            events.emit(Event::Error {
                message: "decode worker unavailable".to_string(),
            });
            set_state(state, events, PlayerState::Stopped);
            return;
        };
        let backend = output_backend_for_selected(state.selected_backend);
        match start_session(StartSessionArgs {
            path,
            decode_worker,
            internal_tx: internal_tx.clone(),
            backend,
            device_id: state.selected_device_id.clone(),
            match_track_sample_rate: state.match_track_sample_rate,
            gapless_playback: state.gapless_playback,
            out_spec,
            start_at_ms: start_at_ms as i64,
            volume: Arc::clone(&state.volume_atomic),
            lfe_mode: state.lfe_mode,
            output_sink_chunk_frames: state.output_sink_chunk_frames,
            output_sink_only: state.desired_output_sink_route.is_some(),
            output_pipeline: &mut state.output_pipeline,
        }) {
            Ok(session) => {
                track_info.store(Some(Arc::new(session.track_info.clone())));
                state.session = Some(session);
                state.pending_session_start = false;
                if let Err(message) = sync_output_sink_with_active_session(state, internal_tx) {
                    events.emit(Event::Error { message });
                }
                if let Err(message) = apply_dsp_chain(state) {
                    events.emit(Event::Error { message });
                }
                if let Some(session) = state.session.as_ref() {
                    let _ = session.ctrl_tx.send(DecodeCtrl::Play);
                }
                set_state(state, events, PlayerState::Buffering);
            }
            Err(message) => {
                state.pending_session_start = false;
                state.wants_playback = false;
                state.play_request_started_at = None;
                events.emit(Event::Error { message });
                set_state(state, events, PlayerState::Stopped);
            }
        }
    }

    let Some(session) = state.session.as_ref() else {
        return;
    };

    if state.desired_output_sink_route.is_some() {
        session.output_enabled.store(false, Ordering::Release);
        if !state.wants_playback {
            set_state(state, events, PlayerState::Paused);
            return;
        }

        let channels = session.out_channels as usize;
        if channels == 0 {
            return;
        }
        let pending_samples = state
            .output_sink_worker
            .as_ref()
            .map(|worker| worker.pending_samples())
            .unwrap_or(0);
        let pending_frames = pending_samples / channels;
        let buffered_ms =
            ((pending_frames as u64 * 1000) / session.out_sample_rate.max(1) as u64) as i64;
        let (low_watermark_ms, high_watermark_ms) = output_sink_queue_watermarks_ms(
            session.out_sample_rate,
            state.output_sink_chunk_frames,
        );

        match state.player_state {
            PlayerState::Playing => {
                if buffered_ms <= low_watermark_ms {
                    set_state(state, events, PlayerState::Buffering);
                }
            }
            PlayerState::Buffering => {
                if buffered_ms >= high_watermark_ms {
                    if state.seek_track_fade {
                        session
                            .transition_target_gain
                            .store(1.0f32.to_bits(), Ordering::Relaxed);
                    } else {
                        force_transition_gain_unity(Some(session));
                    }
                    state.play_request_started_at = None;
                    set_state(state, events, PlayerState::Playing);
                }
            }
            PlayerState::Paused | PlayerState::Stopped => {}
        }
        return;
    }

    let channels = session.out_channels as usize;
    if channels == 0 {
        return;
    }

    let buffered_samples = session.buffered_samples.load(Ordering::Relaxed);
    let buffered_frames = buffered_samples / channels;
    let buffered_ms =
        ((buffered_frames as u64 * 1000) / session.out_sample_rate.max(1) as u64) as i64;

    let underruns = session.underrun_callbacks.load(Ordering::Relaxed);
    if underruns > state.last_underrun_total
        && state.last_underrun_log_at.elapsed() >= UNDERRUN_LOG_INTERVAL
    {
        let delta = underruns - state.last_underrun_total;
        state.last_underrun_total = underruns;
        state.last_underrun_log_at = Instant::now();
        events.emit(Event::Log {
            message: format!("audio underrun callbacks: total={underruns}, +{delta}"),
        });
    }

    if !state.wants_playback {
        session.output_enabled.store(false, Ordering::Release);
        return;
    }

    let (low_watermark_ms, high_watermark_ms) = match state.selected_backend {
        stellatune_core::AudioBackend::WasapiExclusive => (
            BUFFER_LOW_WATERMARK_MS_EXCLUSIVE,
            BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE,
        ),
        _ => (BUFFER_LOW_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS),
    };

    match state.player_state {
        PlayerState::Playing => {
            if buffered_ms <= low_watermark_ms {
                session.output_enabled.store(false, Ordering::Release);
                set_state(state, events, PlayerState::Buffering);
                debug!("buffer low watermark reached: buffered_ms={buffered_ms}");
            } else {
                session.output_enabled.store(true, Ordering::Release);
            }
        }
        PlayerState::Buffering => {
            if buffered_ms >= high_watermark_ms {
                session.output_enabled.store(true, Ordering::Release);
                if state.seek_track_fade {
                    session
                        .transition_target_gain
                        .store(1.0f32.to_bits(), Ordering::Relaxed);
                } else {
                    force_transition_gain_unity(Some(session));
                }
                set_state(state, events, PlayerState::Playing);
                let elapsed_ms = state
                    .play_request_started_at
                    .take()
                    .map(|t0| t0.elapsed().as_millis() as u64);
                debug!(buffered_ms, elapsed_ms = ?elapsed_ms, "buffering completed");
            } else {
                session.output_enabled.store(false, Ordering::Release);
            }
        }
        PlayerState::Paused | PlayerState::Stopped => {
            session.output_enabled.store(false, Ordering::Release);
        }
    }
}
