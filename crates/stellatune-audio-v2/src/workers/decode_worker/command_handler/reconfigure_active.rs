use std::sync::Arc;

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::runtime::runner::RunnerState;
use crate::runtime::sink_session::SinkActivationMode;
use crate::types::PlayerState;
use crate::workers::decode_worker::command_handler::control_apply;
use crate::workers::decode_worker::command_handler::gain_transition;
use crate::workers::decode_worker::pipeline_policies::apply_decode_policies;
use crate::workers::decode_worker::state::DecodeWorkerState;
use crate::workers::decode_worker::util::update_state;
use crate::workers::decode_worker::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(super) fn handle(
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> Result<(), String> {
    let Some(input) = state.active_input.clone() else {
        return Ok(());
    };

    let resume_playing = state.state == PlayerState::Playing;
    let resume_position_ms = state.ctx.position_ms.max(0);
    let previous_runner = state.runner.take();
    state.reset_context();
    state.prewarmed_next = None;
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;

    let plan = match state.pinned_plan.as_ref() {
        Some(plan) => Arc::clone(plan),
        None => assembler.plan(&input).map_err(|e| e.to_string())?,
    };
    let mut assembled = pipeline_runtime
        .ensure(plan.as_ref())
        .map_err(|e| e.to_string())?;
    apply_decode_policies(&mut assembled, state);
    let build_result = (|| -> Result<_, String> {
        let mut next_runner = assembled
            .into_runner(
                state.sink_latency,
                state.sink_control_timeout,
                Some(Arc::clone(&state.master_gain_hot_control)),
            )
            .map_err(|e| e.to_string())?;
        next_runner
            .prepare_decode(&input, &mut state.ctx)
            .map_err(|e| e.to_string())?;
        next_runner
            .activate_sink(
                &mut state.sink_session,
                &state.ctx,
                SinkActivationMode::ImmediateCutover,
            )
            .map_err(|e| e.to_string())?;
        Ok(next_runner)
    })();
    if let Some(mut previous_runner) = previous_runner {
        previous_runner.stop_decode_only(&mut state.ctx);
    }
    let mut next_runner = build_result?;
    control_apply::apply_master_gain_level_to_runner(
        &mut next_runner,
        &mut state.ctx,
        state.master_gain_hot_control.snapshot().level,
        0,
    )?;
    control_apply::replay_persisted_stage_controls_to_runner(
        &state.persisted_stage_controls,
        &mut next_runner,
        &mut state.ctx,
    )?;
    if resume_position_ms > 0 {
        next_runner
            .seek(resume_position_ms, &mut state.sink_session, &mut state.ctx)
            .map_err(|e| e.to_string())?;
        state.ctx.position_ms = resume_position_ms;
        callback(DecodeWorkerEvent::Position {
            position_ms: resume_position_ms,
        });
    }
    if resume_playing {
        gain_transition::request_fade_in_from_silence_with_runner(
            &mut next_runner,
            &mut state.ctx,
            state.gain_transition,
            state.gain_transition.play_fade_in_ms,
        )
        .map_err(|e| e.to_string())?;
    }
    next_runner.set_state(if resume_playing {
        RunnerState::Playing
    } else {
        RunnerState::Paused
    });
    state.runner = Some(next_runner);
    update_state(
        callback,
        &mut state.state,
        if resume_playing {
            PlayerState::Playing
        } else {
            PlayerState::Paused
        },
    );
    Ok(())
}
