use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::types::PlayerState;
use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tracing::{debug, trace};

use crate::engine::control::control_actor::ControlActor;
use crate::engine::control::{
    BUFFER_HIGH_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE, BUFFER_LOW_WATERMARK_MS,
    BUFFER_LOW_WATERMARK_MS_EXCLUSIVE, BUFFER_RESUME_STABLE_TICKS, DecodeCtrl, EngineState, Event,
    StartSessionArgs, UNDERRUN_LOG_INTERVAL, apply_dsp_chain, debug_metrics,
    decode_worker_unavailable_error_message, ensure_decode_worker, force_transition_gain_unity,
    is_decode_worker_unavailable_error, output_backend_for_selected,
    output_sink_queue_watermarks_ms, resolve_output_spec_and_sink_chunk, restart_decode_worker,
    set_state, start_session, sync_output_sink_with_active_session,
};

pub(crate) struct ControlTickMessage;

impl Message for ControlTickMessage {
    type Response = ();
}

impl Handler<ControlTickMessage> for ControlActor {
    fn handle(&mut self, _message: ControlTickMessage, _ctx: &mut ActorContext<Self>) {
        // If we are waiting for an output spec (prewarm) and have no active session, start the
        // session as soon as the spec becomes available.
        if self.state.session.is_none()
            && self.state.wants_playback
            && self.state.pending_session_start
            && self.state.cached_output_spec.is_some()
        {
            let Some(path) = self.state.current_track.clone() else {
                self.state.pending_session_start = false;
                self.state.wants_playback = false;
                self.state.play_request_started_at = None;
                set_state(&mut self.state, &self.events, PlayerState::Stopped);
                return;
            };
            let Some(cached_out_spec) = self.state.cached_output_spec else {
                self.state.pending_session_start = false;
                self.state.wants_playback = false;
                self.state.play_request_started_at = None;
                self.events.emit(Event::Error {
                    message: "output spec missing while pending session start".to_string(),
                });
                set_state(&mut self.state, &self.events, PlayerState::Stopped);
                return;
            };
            let out_spec =
                match resolve_output_spec_and_sink_chunk(&mut self.state, cached_out_spec) {
                    Ok(spec) => spec,
                    Err(message) => {
                        self.state.pending_session_start = false;
                        self.state.wants_playback = false;
                        self.state.play_request_started_at = None;
                        self.events.emit(Event::Error { message });
                        set_state(&mut self.state, &self.events, PlayerState::Stopped);
                        return;
                    },
                };
            let start_at_ms = self.state.position_ms.max(0) as u64;
            let backend = output_backend_for_selected(self.state.selected_backend);
            let path_for_timing = path.clone();
            let mut start_attempt: u8 = 0;
            let start_result = loop {
                ensure_decode_worker(&mut self.state, &self.internal_tx);
                let Some(decode_worker) = self.state.decode_worker.as_ref() else {
                    break Err(decode_worker_unavailable_error_message(
                        "worker missing from engine state",
                    ));
                };

                let result = start_session(StartSessionArgs {
                    path: path.clone(),
                    decode_worker,
                    internal_tx: self.internal_tx.clone(),
                    backend,
                    device_id: self.state.selected_device_id.clone(),
                    match_track_sample_rate: self.state.match_track_sample_rate,
                    gapless_playback: self.state.gapless_playback,
                    out_spec,
                    start_at_ms: start_at_ms as i64,
                    volume: Arc::clone(&self.state.volume_atomic),
                    lfe_mode: self.state.lfe_mode,
                    output_sink_chunk_frames: self.state.output_sink_chunk_frames,
                    output_sink_only: self.state.desired_output_sink_route.is_some(),
                    resample_quality: self.state.resample_quality,
                    output_pipeline: &mut self.state.output_pipeline,
                });
                match result {
                    Ok(session) => break Ok(session),
                    Err(message)
                        if start_attempt == 0 && is_decode_worker_unavailable_error(&message) =>
                    {
                        restart_decode_worker(&mut self.state, &self.internal_tx, &message);
                        start_attempt = start_attempt.saturating_add(1);
                    },
                    Err(message) => break Err(message),
                }
            };

            match start_result {
                Ok(session) => {
                    self.track_info
                        .store(Some(Arc::new(session.track_info.clone())));
                    self.state.session = Some(session);
                    if let Some(timing) = self.state.manual_switch_timing.as_mut()
                        && timing.to_track == path_for_timing
                    {
                        timing.session_ready_at = Some(Instant::now());
                    }
                    self.state.pending_session_start = false;
                    // Session startup may reuse an existing output pipeline. Always reset transition
                    // gain to unity here so we never inherit a previous disrupt fade target (0.0).
                    force_transition_gain_unity(self.state.session.as_ref());
                    if let Err(message) =
                        sync_output_sink_with_active_session(&mut self.state, &self.internal_tx)
                    {
                        self.events.emit(Event::Error { message });
                    }
                    if let Err(message) = apply_dsp_chain(&mut self.state) {
                        self.events.emit(Event::Error { message });
                    }
                    if let Some(session) = self.state.session.as_ref() {
                        let _ = session.ctrl_tx.send(DecodeCtrl::Play);
                    }
                    set_state(&mut self.state, &self.events, PlayerState::Buffering);
                },
                Err(message) => {
                    self.state.pending_session_start = false;
                    self.state.wants_playback = false;
                    self.state.play_request_started_at = None;
                    self.events.emit(Event::Error { message });
                    set_state(&mut self.state, &self.events, PlayerState::Stopped);
                },
            }
        }

        let Some(session) = self.state.session.as_ref() else {
            return;
        };

        if self.state.desired_output_sink_route.is_some() {
            session.output_enabled.store(false, Ordering::Release);
            if !self.state.wants_playback {
                self.state.buffering_ready_streak = 0;
                set_state(&mut self.state, &self.events, PlayerState::Paused);
                return;
            }

            let channels = session.out_channels as usize;
            if channels == 0 {
                return;
            }
            let (pending_samples, sink_runtime_queued_samples) = self
                .state
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
                self.state.output_sink_chunk_frames,
            );

            match self.state.player_state {
                PlayerState::Playing => {
                    if buffered_ms <= low_watermark_ms {
                        set_state(&mut self.state, &self.events, PlayerState::Buffering);
                    }
                },
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
                        self.state.buffering_ready_streak =
                            self.state.buffering_ready_streak.saturating_add(1);
                    } else {
                        self.state.buffering_ready_streak = 0;
                    }
                    if self.state.buffering_ready_streak >= BUFFER_RESUME_STABLE_TICKS {
                        let ready_streak = self.state.buffering_ready_streak;
                        if self.state.seek_track_fade {
                            session
                                .transition_target_gain
                                .store(1.0f32.to_bits(), Ordering::Relaxed);
                        } else {
                            force_transition_gain_unity(Some(session));
                        }
                        self.state.buffering_ready_streak = 0;
                        let elapsed_ms = self
                            .state
                            .play_request_started_at
                            .take()
                            .map(|t0| t0.elapsed().as_millis() as u64);
                        if let Some(elapsed_ms) = elapsed_ms {
                            debug_metrics::note_track_switch_latency(elapsed_ms);
                        }
                        set_state(&mut self.state, &self.events, PlayerState::Playing);
                        maybe_emit_manual_switch_timing(
                            &mut self.state,
                            Instant::now(),
                            buffered_ms,
                        );
                        trace!(
                            buffered_ms,
                            elapsed_ms = ?elapsed_ms,
                            ready_streak,
                            "buffering completed"
                        );
                    }
                },
                PlayerState::Paused | PlayerState::Stopped => {},
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
        if underruns > self.state.last_underrun_total
            && self.state.last_underrun_log_at.elapsed() >= UNDERRUN_LOG_INTERVAL
        {
            let delta = underruns - self.state.last_underrun_total;
            self.state.last_underrun_total = underruns;
            self.state.last_underrun_log_at = Instant::now();
            self.events.emit(Event::Log {
                message: format!("audio underrun callbacks: total={underruns}, +{delta}"),
            });
        }

        if !self.state.wants_playback {
            self.state.buffering_ready_streak = 0;
            session.output_enabled.store(false, Ordering::Release);
            return;
        }

        let (low_watermark_ms, high_watermark_ms) = match self.state.selected_backend {
            crate::types::AudioBackend::WasapiExclusive => (
                BUFFER_LOW_WATERMARK_MS_EXCLUSIVE,
                BUFFER_HIGH_WATERMARK_MS_EXCLUSIVE,
            ),
            _ => (BUFFER_LOW_WATERMARK_MS, BUFFER_HIGH_WATERMARK_MS),
        };

        match self.state.player_state {
            PlayerState::Playing => {
                if buffered_ms <= low_watermark_ms {
                    session.output_enabled.store(false, Ordering::Release);
                    set_state(&mut self.state, &self.events, PlayerState::Buffering);
                    debug!("buffer low watermark reached: buffered_ms={buffered_ms}");
                } else {
                    session.output_enabled.store(true, Ordering::Release);
                }
            },
            PlayerState::Buffering => {
                if buffered_ms >= high_watermark_ms {
                    self.state.buffering_ready_streak =
                        self.state.buffering_ready_streak.saturating_add(1);
                } else {
                    self.state.buffering_ready_streak = 0;
                }
                if self.state.buffering_ready_streak >= BUFFER_RESUME_STABLE_TICKS {
                    let ready_streak = self.state.buffering_ready_streak;
                    session.output_enabled.store(true, Ordering::Release);
                    if self.state.seek_track_fade {
                        session
                            .transition_target_gain
                            .store(1.0f32.to_bits(), Ordering::Relaxed);
                    } else {
                        force_transition_gain_unity(Some(session));
                    }
                    self.state.buffering_ready_streak = 0;
                    set_state(&mut self.state, &self.events, PlayerState::Playing);
                    let elapsed_ms = self
                        .state
                        .play_request_started_at
                        .take()
                        .map(|t0| t0.elapsed().as_millis() as u64);
                    if let Some(elapsed_ms) = elapsed_ms {
                        debug_metrics::note_track_switch_latency(elapsed_ms);
                    }
                    maybe_emit_manual_switch_timing(&mut self.state, Instant::now(), buffered_ms);
                    trace!(
                        buffered_ms,
                        elapsed_ms = ?elapsed_ms,
                        ready_streak,
                        "buffering completed"
                    );
                } else {
                    session.output_enabled.store(false, Ordering::Release);
                }
            },
            PlayerState::Paused | PlayerState::Stopped => {
                session.output_enabled.store(false, Ordering::Release);
            },
        }
    }
}

fn span_ms(from: Option<Instant>, to: Option<Instant>) -> Option<u64> {
    match (from, to) {
        (Some(start), Some(end)) if end >= start => {
            Some(end.duration_since(start).as_millis() as u64)
        },
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
