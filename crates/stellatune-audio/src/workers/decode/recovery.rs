//! Sink-disconnect recovery helpers for the decode worker loop.
//!
//! Recovery is intentionally implemented as a loop-level policy instead of a
//! sink-internal retry to keep error ownership in decode worker state.

use std::sync::Arc;
use std::time::{Duration, Instant};

use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;
use tracing::{info, warn};

use crate::config::engine::EngineConfig;
use crate::error::DecodeError;
use crate::pipeline::assembly::{PipelineAssembler, PipelineRuntime};
use crate::pipeline::runtime::runner::RunnerState;
use crate::pipeline::runtime::sink_session::SinkActivationMode;
use crate::workers::decode::handlers::{
    apply_master_gain_level_to_runner, replay_persisted_stage_controls_to_runner,
};
use crate::workers::decode::pipeline_policies::apply_decode_policies;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

/// Initializes sink-recovery state after disconnect and emits the first retry event.
pub(crate) fn schedule_sink_recovery(
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

/// Executes one recovery tick once backoff elapses.
///
/// Returns `true` while recovery handling should continue in the caller loop.
pub(crate) fn try_sink_recovery_tick(
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

    let message =
        recover_result
            .err()
            .unwrap_or(DecodeError::Pipeline(PipelineError::StageFailure(
                "sink recovery failed".to_string(),
            )));
    warn!(
        attempt,
        message = %message,
        active_input = %active_input_for_log(state),
        "sink recovery attempt failed"
    );
    state.recovery_attempts = attempt;
    if attempt >= state.sink_recovery.max_attempts {
        warn!(
            attempt,
            max_attempts = state.sink_recovery.max_attempts,
            message = %message,
            active_input = %active_input_for_log(state),
            "sink recovery exhausted; stopping playback"
        );
        state.recovery_retry_at = None;
        callback(DecodeWorkerEvent::Error(message));
        return false;
    }
    // Backoff is computed for the next visible retry attempt.
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

pub(crate) fn active_input_for_log(state: &DecodeWorkerState) -> String {
    match state.active_input.as_ref() {
        Some(InputRef::TrackToken(track_token)) => truncate_for_log(track_token, 160),
        None => "<none>".to_string(),
    }
}

/// Reassembles and reactivates the current track runner at the last known position.
fn rebuild_active_runner(
    assembler: &Arc<dyn PipelineAssembler>,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    let input = state
        .active_input
        .clone()
        .ok_or(DecodeError::NoActiveInputForRecovery)?;
    let resume_position_ms = state.ctx.position_ms.max(0);
    let plan = match state.pinned_plan.as_ref() {
        Some(plan) => Arc::clone(plan),
        None => assembler.plan(&input)?,
    };
    let mut assembled = pipeline_runtime.ensure(plan.as_ref())?;
    apply_decode_policies(&mut assembled, state);
    let mut next_ctx = state.fresh_context();
    let mut next_runner =
        assembled.into_runner(Some(Arc::clone(&state.master_gain_hot_control)))?;
    next_runner.prepare_decode(&input, &mut next_ctx)?;
    next_runner.activate_sink(
        &mut state.sink_session,
        &next_ctx,
        SinkActivationMode::ImmediateCutover,
    )?;
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
        // Resume seeks are applied after sink activation so transport and sink
        // observe the same timeline origin for the rebuilt runner.
        next_runner.seek(resume_position_ms, &mut state.sink_session, &mut next_ctx)?;
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

#[cfg(test)]
#[path = "../../tests/workers/decode/recovery/mod.rs"]
mod tests;
