use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;
use tracing::{info, warn};

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::runtime::runner::{RunnerState, StepResult};
use crate::runtime::sink_session::SinkActivationMode;
use crate::runtime::transform::control::SharedMasterGainHotControl;
use crate::types::{EngineConfig, PlayerState, StopBehavior};
use crate::workers::decode_worker::command::DecodeWorkerCommand;
use crate::workers::decode_worker::command_handler::handle_command;
use crate::workers::decode_worker::command_handler::open::open_input;
use crate::workers::decode_worker::command_handler::{
    apply_master_gain_level_to_runner, replay_persisted_stage_controls_to_runner,
    request_fade_in_from_silence_with_runner,
};
use crate::workers::decode_worker::pipeline_policies::apply_decode_policies;
use crate::workers::decode_worker::state::DecodeWorkerState;
use crate::workers::decode_worker::util::{maybe_emit_position, update_state};
use crate::workers::decode_worker::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn decode_worker_main(
    assembler: Arc<dyn PipelineAssembler>,
    config: EngineConfig,
    callback: DecodeWorkerEventCallback,
    rx: Receiver<DecodeWorkerCommand>,
    master_gain_hot_control: SharedMasterGainHotControl,
) {
    let mut pipeline_runtime = assembler.create_runtime();
    let mut state = DecodeWorkerState::new(
        config.sink_latency,
        config.sink_recovery,
        config.gain_transition,
        config.sink_control_timeout,
        master_gain_hot_control,
    );

    loop {
        let timeout = compute_loop_timeout(&state, &config);
        let timeout_rx = crossbeam_channel::after(timeout);

        crossbeam_channel::select_biased! {
            recv(rx) -> msg => {
                match msg {
                    Ok(cmd) => {
                        let should_break =
                            handle_command(cmd, &assembler, &callback, pipeline_runtime.as_mut(), &mut state);
                        if should_break {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            recv(timeout_rx) -> _ => {}
        };

        if state.state != PlayerState::Playing {
            continue;
        }

        if state.runner.is_none() {
            if try_sink_recovery_tick(
                &assembler,
                &callback,
                pipeline_runtime.as_mut(),
                &mut state,
                &config,
            ) {
                continue;
            }
            update_state(&callback, &mut state.state, PlayerState::Stopped);
            continue;
        }

        let step_result = match state.runner.as_mut() {
            Some(active_runner) => active_runner.step(&mut state.sink_session, &mut state.ctx),
            None => Ok(StepResult::Idle),
        };

        match step_result {
            Ok(StepResult::Produced { .. }) => {
                maybe_emit_position(&callback, &state.ctx, &mut state.last_position_emit_at);
            },
            Ok(StepResult::Idle) => {
                std::thread::yield_now();
            },
            Ok(StepResult::Eof) => {
                if let Some(prewarmed_next) = state.prewarmed_next.take() {
                    if let Some(active_runner) = state.runner.as_mut() {
                        let _ = active_runner
                            .drain_sink_for_reuse(&mut state.sink_session, &mut state.ctx);
                        active_runner.stop_decode_only(&mut state.ctx);
                    }
                    state.runner = None;
                    state.queued_next_input = None;
                    let promote_result =
                        promote_prewarmed_next(prewarmed_next, &callback, &mut state);
                    if let Err(message) = promote_result {
                        warn!(message, "failed to promote prewarmed next track");
                        update_state(&callback, &mut state.state, PlayerState::Stopped);
                        callback(DecodeWorkerEvent::Error(message));
                    }
                } else if let Some(next_input) = state.queued_next_input.take() {
                    let open_result = open_input(
                        next_input,
                        true,
                        &assembler,
                        &callback,
                        pipeline_runtime.as_mut(),
                        &mut state,
                    );
                    if let Err(message) = open_result {
                        warn!(message, "failed to open queued next track");
                        update_state(&callback, &mut state.state, PlayerState::Stopped);
                        callback(DecodeWorkerEvent::Error(message));
                    }
                } else {
                    if let Some(active_runner) = state.runner.as_mut() {
                        let _ = active_runner.stop_with_behavior(
                            StopBehavior::DrainSink,
                            &mut state.sink_session,
                            &mut state.ctx,
                        );
                    }
                    state.runner = None;
                    state.reset_context();
                    state.active_input = None;
                    update_state(&callback, &mut state.state, PlayerState::Stopped);
                    callback(DecodeWorkerEvent::Eof);
                }
            },
            Err(error) => {
                let message = error.to_string();
                let active_input = active_input_for_log(&state);
                if matches!(error, PipelineError::SinkDisconnected) {
                    warn!(
                        message,
                        active_input = %active_input,
                        "sink disconnected, entering recovery"
                    );
                    if let Some(active_runner) = state.runner.as_mut() {
                        active_runner.stop(&mut state.sink_session, &mut state.ctx);
                    }
                    state.runner = None;
                    if schedule_sink_recovery(&callback, &mut state) {
                        continue;
                    }
                } else {
                    warn!(
                        message,
                        active_input = %active_input,
                        "decode worker step failed"
                    );
                }
                if let Some(active_runner) = state.runner.as_mut() {
                    active_runner.stop(&mut state.sink_session, &mut state.ctx);
                }
                state.runner = None;
                state.reset_context();
                state.active_input = None;
                state.queued_next_input = None;
                state.prewarmed_next = None;
                state.recovery_attempts = 0;
                state.recovery_retry_at = None;
                update_state(&callback, &mut state.state, PlayerState::Stopped);
                callback(DecodeWorkerEvent::Error(message));
            },
        }
    }

    if let Some(mut active_runner) = state.runner {
        active_runner.stop(&mut state.sink_session, &mut state.ctx);
    } else {
        state.sink_session.shutdown(false);
    }
}

fn promote_prewarmed_next(
    mut prewarmed_next: crate::workers::decode_worker::state::PrewarmedNext,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> Result<(), String> {
    prewarmed_next
        .runner
        .activate_sink(
            &mut state.sink_session,
            &prewarmed_next.ctx,
            SinkActivationMode::PreserveQueued,
        )
        .map_err(|e| e.to_string())?;
    apply_master_gain_level_to_runner(
        &mut prewarmed_next.runner,
        &mut prewarmed_next.ctx,
        state.master_gain_hot_control.snapshot().level,
        0,
    )?;
    replay_persisted_stage_controls_to_runner(
        &state.persisted_stage_controls,
        &mut prewarmed_next.runner,
        &mut prewarmed_next.ctx,
    )?;
    request_fade_in_from_silence_with_runner(
        &mut prewarmed_next.runner,
        &mut prewarmed_next.ctx,
        state.gain_transition,
        state.gain_transition.open_fade_in_ms,
    )
    .map_err(|e| e.to_string())?;
    prewarmed_next.runner.set_state(RunnerState::Playing);

    state.ctx = prewarmed_next.ctx;
    state.active_input = Some(prewarmed_next.input.clone());
    state.runner = Some(prewarmed_next.runner);
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;
    state.last_position_emit_at = Instant::now();
    callback(DecodeWorkerEvent::Position { position_ms: 0 });
    match prewarmed_next.input {
        stellatune_audio_core::pipeline::context::InputRef::TrackToken(track_token) => {
            callback(DecodeWorkerEvent::TrackChanged { track_token });
        },
    }
    update_state(callback, &mut state.state, PlayerState::Playing);
    Ok(())
}

fn compute_loop_timeout(state: &DecodeWorkerState, config: &EngineConfig) -> Duration {
    if state.state != PlayerState::Playing {
        return config.decode_idle_sleep;
    }

    if let Some(retry_at) = state.recovery_retry_at {
        let until_retry = retry_at.saturating_duration_since(Instant::now());
        return until_retry.min(config.decode_playing_pending_block_sleep);
    }

    if let Some(active_runner) = state.runner.as_ref() {
        if active_runner.has_pending_sink_block() {
            return config.decode_playing_pending_block_sleep;
        }
    }
    config.decode_playing_idle_sleep
}

fn schedule_sink_recovery(
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> bool {
    if state.active_input.is_none() {
        warn!("sink recovery skipped: no active input");
        return false;
    }
    if state.sink_recovery.max_attempts == 0 {
        warn!(
            active_input = %active_input_for_log(state),
            "sink recovery skipped: max_attempts is 0"
        );
        return false;
    }
    state.recovery_attempts = 0;
    let backoff = state.sink_recovery.initial_backoff;
    state.recovery_retry_at = Some(Instant::now() + backoff);
    warn!(
        active_input = %active_input_for_log(state),
        max_attempts = state.sink_recovery.max_attempts,
        initial_backoff_ms = backoff.as_millis() as u64,
        "sink recovery scheduled"
    );
    callback(DecodeWorkerEvent::Recovering {
        attempt: 1,
        backoff_ms: backoff.as_millis() as u64,
    });
    true
}

fn try_sink_recovery_tick(
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
    config: &EngineConfig,
) -> bool {
    let Some(retry_at) = state.recovery_retry_at else {
        return false;
    };
    if Instant::now() < retry_at {
        return true;
    }

    let attempt = state.recovery_attempts.saturating_add(1);
    if attempt > state.sink_recovery.max_attempts {
        warn!(
            active_input = %active_input_for_log(state),
            max_attempts = state.sink_recovery.max_attempts,
            "sink recovery exhausted before retry tick"
        );
        state.recovery_retry_at = None;
        state.recovery_attempts = 0;
        return false;
    }

    let recover_result = rebuild_active_runner(assembler, pipeline_runtime, state);
    if recover_result.is_ok() {
        info!(
            attempt,
            active_input = %active_input_for_log(state),
            position_ms = state.ctx.position_ms,
            "sink recovery succeeded"
        );
        state.recovery_retry_at = None;
        state.recovery_attempts = 0;
        return true;
    }

    let message = recover_result
        .err()
        .unwrap_or_else(|| "sink recovery failed".to_string());
    warn!(
        attempt,
        message,
        active_input = %active_input_for_log(state),
        "sink recovery attempt failed"
    );
    state.recovery_attempts = attempt;
    if attempt >= state.sink_recovery.max_attempts {
        warn!(
            attempt,
            max_attempts = state.sink_recovery.max_attempts,
            message,
            active_input = %active_input_for_log(state),
            "sink recovery exhausted; stopping playback"
        );
        state.recovery_retry_at = None;
        callback(DecodeWorkerEvent::Error(message));
        return false;
    }
    let next_attempt = attempt.saturating_add(1);
    let backoff = compute_recovery_backoff(config, next_attempt);
    warn!(
        attempt = next_attempt,
        backoff_ms = backoff.as_millis() as u64,
        active_input = %active_input_for_log(state),
        "sink recovery retry scheduled"
    );
    state.recovery_retry_at = Some(Instant::now() + backoff);
    callback(DecodeWorkerEvent::Recovering {
        attempt: next_attempt,
        backoff_ms: backoff.as_millis() as u64,
    });
    true
}

fn rebuild_active_runner(
    assembler: &Arc<dyn PipelineAssembler>,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> Result<(), String> {
    let input = state
        .active_input
        .clone()
        .ok_or_else(|| "no active input for sink recovery".to_string())?;
    let resume_position_ms = state.ctx.position_ms.max(0);
    let plan = match state.pinned_plan.as_ref() {
        Some(plan) => Arc::clone(plan),
        None => assembler.plan(&input).map_err(|e| e.to_string())?,
    };
    let mut assembled = pipeline_runtime
        .ensure(plan.as_ref())
        .map_err(|e| e.to_string())?;
    apply_decode_policies(&mut assembled, state);
    let mut next_ctx = state.fresh_context();
    let mut next_runner = assembled
        .into_runner(
            state.sink_latency,
            state.sink_control_timeout,
            Some(Arc::clone(&state.master_gain_hot_control)),
        )
        .map_err(|e| e.to_string())?;
    next_runner
        .prepare_decode(&input, &mut next_ctx)
        .map_err(|e| e.to_string())?;
    next_runner
        .activate_sink(
            &mut state.sink_session,
            &next_ctx,
            SinkActivationMode::ImmediateCutover,
        )
        .map_err(|e| e.to_string())?;
    apply_master_gain_level_to_runner(
        &mut next_runner,
        &mut next_ctx,
        state.master_gain_hot_control.snapshot().level,
        0,
    )?;
    replay_persisted_stage_controls_to_runner(
        &state.persisted_stage_controls,
        &mut next_runner,
        &mut next_ctx,
    )?;
    if resume_position_ms > 0 {
        next_runner
            .seek(resume_position_ms, &mut state.sink_session, &mut next_ctx)
            .map_err(|e| e.to_string())?;
        next_ctx.position_ms = resume_position_ms;
    }
    next_runner.set_state(RunnerState::Playing);
    state.ctx = next_ctx;
    state.runner = Some(next_runner);
    state.last_position_emit_at = Instant::now();
    Ok(())
}

fn compute_recovery_backoff(config: &EngineConfig, attempt: u32) -> Duration {
    if attempt <= 1 {
        return config.sink_recovery.initial_backoff;
    }
    let exp = (attempt - 1).min(16);
    let factor = 1u128 << exp;
    let initial_ms = config.sink_recovery.initial_backoff.as_millis();
    let scaled_ms = initial_ms.saturating_mul(factor);
    let bounded_ms = scaled_ms.min(config.sink_recovery.max_backoff.as_millis());
    Duration::from_millis(bounded_ms as u64)
}

fn active_input_for_log(state: &DecodeWorkerState) -> String {
    match state.active_input.as_ref() {
        Some(InputRef::TrackToken(track_token)) => truncate_for_log(track_token, 160),
        None => "<none>".to_string(),
    }
}

fn truncate_for_log(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return "...".to_string();
    }
    let mut out = String::with_capacity(text.len().min(max_chars.saturating_add(3)));
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}
