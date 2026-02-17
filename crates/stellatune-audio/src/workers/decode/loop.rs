//! Main decode worker loop and EOF transition helpers.
//!
//! # Loop Structure
//!
//! The loop alternates between:
//! - command intake (biased, so control requests are processed promptly),
//! - timed wake-ups for runner stepping and recovery retries.
//!
//! This keeps state transitions deterministic while still allowing periodic
//! forward progress even when no command is arriving.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use stellatune_audio_core::pipeline::error::PipelineError;
use tracing::warn;

use crate::config::engine::{EngineConfig, PlayerState, StopBehavior};
use crate::error::DecodeError;
use crate::pipeline::assembly::PipelineAssembler;
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;
use crate::pipeline::runtime::runner::{RunnerState, StepResult};
use crate::pipeline::runtime::sink_session::SinkActivationMode;
use crate::workers::decode::command::DecodeWorkerCommand;
use crate::workers::decode::handlers::handle_command;
use crate::workers::decode::handlers::open::open_input;
use crate::workers::decode::handlers::{
    apply_master_gain_level_to_runner, replay_persisted_stage_controls_to_runner,
    request_fade_in_from_silence_with_runner,
};
use crate::workers::decode::recovery;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::{maybe_emit_position, update_state};
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

/// Runs the decode worker event loop until shutdown or channel closure.
///
/// The loop prioritizes control commands, drives runner stepping while playing,
/// and coordinates EOF promotion, queued-next fallback, and sink recovery.
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
            // Periodic wake drives playback stepping and recovery retries.
            recv(timeout_rx) -> _ => {}
        };

        if state.state != PlayerState::Playing {
            continue;
        }

        if state.runner.is_none() {
            if recovery::try_sink_recovery_tick(
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
                    // Promote already-prepared next runner for a cheap cutover.
                    if let Some(active_runner) = state.runner.as_mut() {
                        let _ = active_runner
                            .drain_sink_for_reuse(&mut state.sink_session, &mut state.ctx);
                        active_runner.stop_decode_only(&mut state.ctx);
                    }
                    state.runner = None;
                    state.queued_next_input = None;
                    let promote_result =
                        promote_prewarmed_next(prewarmed_next, &callback, &mut state);
                    if let Err(error) = promote_result {
                        warn!(message = %error, "failed to promote prewarmed next track");
                        update_state(&callback, &mut state.state, PlayerState::Stopped);
                        callback(DecodeWorkerEvent::Error(error));
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
                    if let Err(error) = open_result {
                        warn!(message = %error, "failed to open queued next track");
                        update_state(&callback, &mut state.state, PlayerState::Stopped);
                        callback(DecodeWorkerEvent::Error(error));
                    }
                } else {
                    // Fully drained EOF path with no fallback candidate.
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
                let active_input = recovery::active_input_for_log(&state);
                if matches!(error, PipelineError::SinkDisconnected) {
                    // Sink disconnection is recoverable; stage failure is not.
                    warn!(
                        message = %error,
                        active_input = %active_input,
                        "sink disconnected, entering recovery"
                    );
                    if let Some(active_runner) = state.runner.as_mut() {
                        active_runner.stop(&mut state.sink_session, &mut state.ctx);
                    }
                    state.runner = None;
                    if recovery::schedule_sink_recovery(&callback, &mut state) {
                        continue;
                    }
                } else {
                    warn!(
                        message = %error,
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
                callback(DecodeWorkerEvent::Error(DecodeError::Pipeline(error)));
            },
        }
    }

    if let Some(mut active_runner) = state.runner {
        active_runner.stop(&mut state.sink_session, &mut state.ctx);
    } else {
        state.sink_session.shutdown(false);
    }
}

/// Promotes a prewarmed runner into the active playback slot.
///
/// This preserves sink routing, reapplies persisted stage controls, and emits
/// track/state notifications as part of the cutover.
fn promote_prewarmed_next(
    mut prewarmed_next: crate::workers::decode::state::PrewarmedNext,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    prewarmed_next.runner.activate_sink(
        &mut state.sink_session,
        &prewarmed_next.ctx,
        SinkActivationMode::PreserveQueued,
    )?;
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
    )?;
    prewarmed_next.runner.set_state(RunnerState::Playing);

    state.ctx = prewarmed_next.ctx;
    state.active_input = Some(prewarmed_next.input.clone());
    state.runner = Some(prewarmed_next.runner);
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;
    state.last_position_emit_at = Instant::now();
    // Cutover always starts from the new input origin in the promoted context.
    callback(DecodeWorkerEvent::Position { position_ms: 0 });
    match prewarmed_next.input {
        stellatune_audio_core::pipeline::context::InputRef::TrackToken(track_token) => {
            callback(DecodeWorkerEvent::TrackChanged { track_token });
        },
    }
    update_state(callback, &mut state.state, PlayerState::Playing);
    Ok(())
}

/// Computes the next loop wait duration based on playback and recovery state.
fn compute_loop_timeout(state: &DecodeWorkerState, config: &EngineConfig) -> Duration {
    if state.state != PlayerState::Playing {
        return config.decode_idle_sleep;
    }

    if let Some(retry_at) = state.recovery_retry_at {
        let until_retry = retry_at.saturating_duration_since(Instant::now());
        return until_retry.min(config.decode_playing_pending_block_sleep);
    }

    if let Some(active_runner) = state.runner.as_ref()
        && active_runner.has_pending_sink_block()
    {
        return config.decode_playing_pending_block_sleep;
    }
    config.decode_playing_idle_sleep
}

#[cfg(test)]
#[path = "../../tests/workers/decode/loop_timeout.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/workers/decode/loop/mod.rs"]
mod loop_tests;
