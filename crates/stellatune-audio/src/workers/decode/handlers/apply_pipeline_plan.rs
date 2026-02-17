use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::config::engine::PlayerState;
use crate::pipeline::assembly::{PipelinePlan, PipelineRuntime};
use crate::pipeline::runtime::runner::RunnerState;
use crate::pipeline::runtime::sink_session::SinkActivationMode;
use crate::workers::decode::handlers::control_apply;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::pipeline_policies::apply_decode_policies;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn handle(
    plan: Arc<dyn PipelinePlan>,
    resp_tx: Sender<Result<(), String>>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    state.pinned_plan = Some(Arc::clone(&plan));
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

    let result = (|| {
        let mut assembled = pipeline_runtime
            .ensure(plan.as_ref())
            .map_err(|e| e.to_string())?;
        apply_decode_policies(&mut assembled, state);
        let mut next_runner = assembled
            .into_runner(Some(Arc::clone(&state.master_gain_hot_control)))
            .map_err(|e| e.to_string())?;
        next_runner
            .prepare_decode(&input, &mut state.ctx)
            .map_err(|e| e.to_string())?;
        next_runner
            .activate_sink(
                &mut state.sink_session,
                &state.ctx,
                SinkActivationMode::ForceRecreate,
            )
            .map_err(|e| e.to_string())?;
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
                transition,
                transition.play_fade_in_ms,
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
        state.recovery_attempts = 0;
        state.recovery_retry_at = None;
        Ok(())
    })();
    if result.is_err() {
        update_state(callback, &mut state.state, PlayerState::Stopped);
    }
    let _ = resp_tx.send(result);
    false
}
