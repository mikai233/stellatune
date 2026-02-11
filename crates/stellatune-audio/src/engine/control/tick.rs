use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Instant;

use crossbeam_channel::Sender;
use tracing::{debug, trace};

use stellatune_core::{Event, HostEventTopic, HostPlayerTickPayload, PlayerState};
use stellatune_output::output_spec_for_device;

use super::{
    BUFFER_HIGH_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE, BUFFER_LOW_WATERMARK_MS,
    BUFFER_LOW_WATERMARK_MS_EXCLUSIVE, BUFFER_RESUME_STABLE_TICKS, DecodeCtrl, EngineState,
    EventHub, InternalMsg, SharedTrackInfo, StartSessionArgs, UNDERRUN_LOG_INTERVAL,
    apply_dsp_chain, debug_metrics, decode_worker_unavailable_error_message, ensure_decode_worker,
    force_transition_gain_unity, is_decode_worker_unavailable_error,
    output_sink_queue_watermarks_ms, output_spec_for_plugin_sink,
    resolve_output_spec_and_sink_chunk, restart_decode_worker, set_state, start_session,
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
        let backend = output_backend_for_selected(state.selected_backend);
        let path_for_timing = path.clone();
        let mut start_attempt: u8 = 0;
        let start_result = loop {
            ensure_decode_worker(state, internal_tx);
            let Some(decode_worker) = state.decode_worker.as_ref() else {
                break Err(decode_worker_unavailable_error_message(
                    "worker missing from engine state",
                ));
            };

            let result = start_session(StartSessionArgs {
                path: path.clone(),
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
            });
            match result {
                Ok(session) => break Ok(session),
                Err(message)
                    if start_attempt == 0 && is_decode_worker_unavailable_error(&message) =>
                {
                    restart_decode_worker(state, internal_tx, &message);
                    start_attempt = start_attempt.saturating_add(1);
                }
                Err(message) => break Err(message),
            }
        };

        match start_result {
            Ok(session) => {
                track_info.store(Some(Arc::new(session.track_info.clone())));
                state.session = Some(session);
                if let Some(timing) = state.manual_switch_timing.as_mut()
                    && timing.to_track == path_for_timing
                {
                    timing.session_ready_at = Some(Instant::now());
                }
                state.pending_session_start = false;
                // Session startup may reuse an existing output pipeline. Always reset transition
                // gain to unity here so we never inherit a previous disrupt fade target (0.0).
                force_transition_gain_unity(state.session.as_ref());
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
            state.buffering_ready_streak = 0;
            set_state(state, events, PlayerState::Paused);
            return;
        }

        let channels = session.out_channels as usize;
        if channels == 0 {
            return;
        }
        let (pending_samples, sink_runtime_queued_samples) = state
            .output_sink_worker
            .as_ref()
            .map(|worker| {
                (
                    worker.pending_samples(),
                    worker.sink_runtime_queued_samples(),
                )
            })
            .unwrap_or((0, 0));
        let effective_buffered_samples =
            pending_samples.saturating_add(sink_runtime_queued_samples);
        let pending_frames = effective_buffered_samples / channels;
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
                let transition_target =
                    f32::from_bits(session.transition_target_gain.load(Ordering::Relaxed));
                let resume_threshold_ms = if transition_target <= 0.01 {
                    // During seek/switch disrupt fade, target may still be muted.
                    // Allow resume once low watermark is stably reached to avoid mute lock.
                    low_watermark_ms.max(1)
                } else {
                    high_watermark_ms
                };
                if buffered_ms >= resume_threshold_ms {
                    state.buffering_ready_streak = state.buffering_ready_streak.saturating_add(1);
                } else {
                    state.buffering_ready_streak = 0;
                }
                if state.buffering_ready_streak >= BUFFER_RESUME_STABLE_TICKS {
                    let ready_streak = state.buffering_ready_streak;
                    if state.seek_track_fade {
                        session
                            .transition_target_gain
                            .store(1.0f32.to_bits(), Ordering::Relaxed);
                    } else {
                        force_transition_gain_unity(Some(session));
                    }
                    state.buffering_ready_streak = 0;
                    let elapsed_ms = state
                        .play_request_started_at
                        .take()
                        .map(|t0| t0.elapsed().as_millis() as u64);
                    if let Some(elapsed_ms) = elapsed_ms {
                        debug_metrics::note_track_switch_latency(elapsed_ms);
                    }
                    set_state(state, events, PlayerState::Playing);
                    maybe_emit_manual_switch_timing(state, Instant::now(), buffered_ms);
                    trace!(
                        buffered_ms,
                        elapsed_ms = ?elapsed_ms,
                        ready_streak,
                        "buffering completed"
                    );
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
        state.buffering_ready_streak = 0;
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
                state.buffering_ready_streak = state.buffering_ready_streak.saturating_add(1);
            } else {
                state.buffering_ready_streak = 0;
            }
            if state.buffering_ready_streak >= BUFFER_RESUME_STABLE_TICKS {
                let ready_streak = state.buffering_ready_streak;
                session.output_enabled.store(true, Ordering::Release);
                if state.seek_track_fade {
                    session
                        .transition_target_gain
                        .store(1.0f32.to_bits(), Ordering::Relaxed);
                } else {
                    force_transition_gain_unity(Some(session));
                }
                state.buffering_ready_streak = 0;
                set_state(state, events, PlayerState::Playing);
                let elapsed_ms = state
                    .play_request_started_at
                    .take()
                    .map(|t0| t0.elapsed().as_millis() as u64);
                if let Some(elapsed_ms) = elapsed_ms {
                    debug_metrics::note_track_switch_latency(elapsed_ms);
                }
                maybe_emit_manual_switch_timing(state, Instant::now(), buffered_ms);
                trace!(
                    buffered_ms,
                    elapsed_ms = ?elapsed_ms,
                    ready_streak,
                    "buffering completed"
                );
            } else {
                session.output_enabled.store(false, Ordering::Release);
            }
        }
        PlayerState::Paused | PlayerState::Stopped => {
            session.output_enabled.store(false, Ordering::Release);
        }
    }
}

fn span_ms(from: Option<Instant>, to: Option<Instant>) -> Option<u64> {
    match (from, to) {
        (Some(start), Some(end)) if end >= start => {
            Some(end.duration_since(start).as_millis() as u64)
        }
        _ => None,
    }
}

fn maybe_emit_manual_switch_timing(state: &mut EngineState, resumed_at: Instant, buffered_ms: i64) {
    let Some(timing) = state.manual_switch_timing.take() else {
        return;
    };
    if state.current_track.as_deref() != Some(timing.to_track.as_str()) {
        state.manual_switch_timing = Some(timing);
        return;
    }

    let wall_total_ms = resumed_at.duration_since(timing.began_at).as_millis() as u64;
    let fade_wait_ms = span_ms(Some(timing.began_at), timing.fade_done_at);
    let stop_after_fade_ms = span_ms(timing.fade_done_at, timing.stop_done_at);
    let commit_after_stop_ms = span_ms(timing.stop_done_at, timing.committed_at);
    let wait_play_request_ms = span_ms(timing.committed_at, timing.play_requested_at);
    let session_prepare_ms = span_ms(timing.play_requested_at, timing.session_ready_at);
    let buffering_wait_ms = span_ms(timing.session_ready_at, Some(resumed_at));
    let play_to_audible_ms = span_ms(timing.play_requested_at, Some(resumed_at));

    debug!(
        switch_id = timing.id,
        from_track = timing.from_track.as_deref().unwrap_or("<none>"),
        to_track = timing.to_track.as_str(),
        fade_wait_ms = ?fade_wait_ms,
        stop_after_fade_ms = ?stop_after_fade_ms,
        commit_after_stop_ms = ?commit_after_stop_ms,
        pre_play_idle_ms = ?wait_play_request_ms,
        session_prepare_ms = ?session_prepare_ms,
        buffering_wait_ms = ?buffering_wait_ms,
        play_to_audible_ms = ?play_to_audible_ms,
        buffered_ms_at_resume = buffered_ms,
        "manual track switch timing summary"
    );
    trace!(
        switch_id = timing.id,
        wall_total_ms, "manual track switch wall timing"
    );
}
