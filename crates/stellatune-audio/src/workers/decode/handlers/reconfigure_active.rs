use std::sync::Arc;

use crate::config::engine::PlayerState;
use crate::error::{DecodeError, NoActivePipelineReason};
use crate::pipeline::assembly::PipelineFactory;
use crate::pipeline::runtime::runner::RunnerState;
use crate::pipeline::runtime::sink_session::SinkActivationMode;
use crate::workers::decode::handlers::control_apply;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::pipeline_policies::apply_decode_policies;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn handle(
    factory: &Arc<dyn PipelineFactory>,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    let Some(input) = state.active_input.clone() else {
        return Ok(());
    };

    let resume_playing = state.pumping;
    let resume_position_ms = state.ctx.position_ms.max(0);
    let previous_runner = state.runner.take();
    state.reset_context();
    state.prewarmed_next = None;
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;
    if let Some(mut previous_runner) = previous_runner {
        previous_runner.stop_decode_only(&mut state.ctx);
    }

    let mut failure_context = "reconfigure_active.assemble";
    let result = (|| -> Result<(), DecodeError> {
        failure_context = "reconfigure_active.assemble";
        let mut assembled = factory.build_pipeline(&input)?;
        apply_decode_policies(&mut assembled, state);
        failure_context = "reconfigure_active.into_runner";
        let build_result = (|| -> Result<_, DecodeError> {
            let mut next_runner =
                assembled.into_runner(Some(Arc::clone(&state.master_gain_hot_control)))?;
            failure_context = "reconfigure_active.prepare_decode";
            next_runner.prepare_decode(&input, &mut state.ctx)?;
            failure_context = "reconfigure_active.activate_sink";
            next_runner.activate_sink(
                &mut state.sink_session,
                &state.ctx,
                SinkActivationMode::ImmediateCutover,
            )?;
            Ok(next_runner)
        })();
        let mut next_runner = build_result?;
        failure_context = "reconfigure_active.apply_master_gain";
        control_apply::apply_master_gain_level_to_runner(
            &mut next_runner,
            &mut state.ctx,
            state.master_gain_hot_control.snapshot().level,
            0,
        )?;
        if resume_position_ms > 0 {
            failure_context = "reconfigure_active.seek";
            next_runner.seek(resume_position_ms, &mut state.sink_session, &mut state.ctx)?;
            state.ctx.position_ms = resume_position_ms;
            callback(DecodeWorkerEvent::Position {
                position_ms: resume_position_ms,
            });
        }
        if resume_playing {
            failure_context = "reconfigure_active.fade_in";
            gain_transition::request_fade_in_from_silence_with_runner(
                &mut next_runner,
                &mut state.ctx,
                state.gain_transition,
                state.gain_transition.play_fade_in_ms,
            )?;
        }
        next_runner.set_state(if resume_playing {
            RunnerState::Playing
        } else {
            RunnerState::Paused
        });
        state.runner = Some(next_runner);
        state.clear_pipeline_unavailable_reason();
        update_state(
            callback,
            &mut state.pumping,
            if resume_playing {
                PlayerState::Playing
            } else {
                PlayerState::Paused
            },
        );
        Ok(())
    })();
    if let Err(error) = &result {
        state.set_pipeline_unavailable_reason(NoActivePipelineReason::PipelineRebuildFailed {
            context: failure_context,
            error: error.to_string(),
        });
    }
    result
}
