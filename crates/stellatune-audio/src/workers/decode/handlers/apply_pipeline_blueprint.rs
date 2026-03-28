use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::config::engine::PlayerState;
use crate::error::{DecodeError, NoActivePipelineReason};
use crate::pipeline::assembly::{PipelineBlueprint, PipelineRuntime};
use crate::pipeline::runtime::runner::RunnerState;
use crate::pipeline::runtime::sink_session::SinkActivationMode;
use crate::workers::decode::handlers::control_apply;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::pipeline_policies::apply_decode_policies;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn handle(
    blueprint: Arc<dyn PipelineBlueprint>,
    resp_tx: Sender<Result<(), DecodeError>>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    state.pinned_blueprint = Some(Arc::clone(&blueprint));
    let Some(input) = state.active_input.clone() else {
        let _ = resp_tx.send(Ok(()));
        return false;
    };

    let transition = state.gain_transition;
    let resume_playing = state.state == PlayerState::Playing;
    let resume_position_ms = state.ctx.position_ms.max(0);
    if let Some(active_runner) = state.runner.as_mut() {
        active_runner.stop_decode_only(&mut state.ctx);
    }
    state.runner = None;
    state.reset_context();
    state.prewarmed_next = None;

    let mut failure_context = "apply_pipeline_blueprint.assemble";
    let result = (|| -> Result<(), DecodeError> {
        failure_context = "apply_pipeline_blueprint.assemble";
        let mut assembled = pipeline_runtime.assemble(blueprint.as_ref())?;
        apply_decode_policies(&mut assembled, state);
        failure_context = "apply_pipeline_blueprint.into_runner";
        let mut next_runner =
            assembled.into_runner(Some(Arc::clone(&state.master_gain_hot_control)))?;
        failure_context = "apply_pipeline_blueprint.prepare_decode";
        next_runner.prepare_decode(&input, &mut state.ctx)?;
        failure_context = "apply_pipeline_blueprint.activate_sink";
        next_runner.activate_sink(
            &mut state.sink_session,
            &state.ctx,
            SinkActivationMode::ForceRecreate,
        )?;
        failure_context = "apply_pipeline_blueprint.apply_master_gain";
        control_apply::apply_master_gain_level_to_runner(
            &mut next_runner,
            &mut state.ctx,
            state.master_gain_hot_control.snapshot().level,
            0,
        )?;
        failure_context = "apply_pipeline_blueprint.replay_stage_updates";
        control_apply::replay_persisted_stage_runtime_updates_to_runner(
            &state.persisted_stage_runtime_updates,
            &mut next_runner,
            Some(&state.sink_session),
            &mut state.ctx,
        )?;
        if resume_position_ms > 0 {
            failure_context = "apply_pipeline_blueprint.seek";
            next_runner.seek(resume_position_ms, &mut state.sink_session, &mut state.ctx)?;
            state.ctx.position_ms = resume_position_ms;
            callback(DecodeWorkerEvent::Position {
                position_ms: resume_position_ms,
            });
        }
        if resume_playing {
            failure_context = "apply_pipeline_blueprint.fade_in";
            gain_transition::request_fade_in_from_silence_with_runner(
                &mut next_runner,
                &mut state.ctx,
                transition,
                transition.play_fade_in_ms,
            )?;
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
        state.recovery_attempts = 0;
        state.recovery_retry_at = None;
        Ok(())
    })();
    if let Err(error) = &result {
        state.set_pipeline_unavailable_reason(NoActivePipelineReason::PipelineRebuildFailed {
            context: failure_context,
            error: error.to_string(),
        });
        update_state(callback, &mut state.state, PlayerState::Stopped);
    } else {
        state.clear_pipeline_unavailable_reason();
    }
    let _ = resp_tx.send(result);
    false
}
