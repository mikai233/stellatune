use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::assembly::{PipelinePlan, PipelineRuntime};
use crate::runtime::runner::RunnerState;
use crate::types::PlayerState;
use crate::worker::decode_loop::command_handler::control_apply;
use crate::worker::decode_loop::command_handler::gain_transition;
use crate::worker::decode_loop::loop_state::DecodeLoopState;
use crate::worker::decode_loop::pipeline_policies::apply_decode_policies;
use crate::worker::decode_loop::util::update_state;
use crate::worker::decode_loop::{DecodeLoopEvent, DecodeLoopEventCallback};

pub(crate) fn handle(
    plan: Arc<dyn PipelinePlan>,
    resp_tx: Sender<Result<(), String>>,
    callback: &DecodeLoopEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
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
        active_runner.stop(&mut state.ctx);
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
            .into_runner(state.sink_latency, state.sink_control_timeout)
            .map_err(|e| e.to_string())?;
        next_runner
            .prepare(&input, &mut state.ctx)
            .map_err(|e| e.to_string())?;
        control_apply::apply_master_gain_level_to_runner(
            &mut next_runner,
            &mut state.ctx,
            state.master_gain_level,
        )?;
        control_apply::replay_persisted_stage_controls_to_runner(
            &state.persisted_stage_controls,
            &mut next_runner,
            &mut state.ctx,
        )?;
        if resume_position_ms > 0 {
            next_runner
                .seek(resume_position_ms, &mut state.ctx)
                .map_err(|e| e.to_string())?;
            state.ctx.position_ms = resume_position_ms;
            callback(DecodeLoopEvent::Position {
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
