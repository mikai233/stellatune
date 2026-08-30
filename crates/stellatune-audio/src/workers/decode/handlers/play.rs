use crate::config::engine::PlayerState;
use crate::error::DecodeError;
use crate::pipeline::runtime::runner::RunnerState;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;

pub(crate) fn handle(
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    let transition = state.gain_transition;
    if let Some(active_runner) = state.runner.as_mut() {
        if !state.pumping {
            if let Err(error) = gain_transition::request_fade_in_with_runner(
                active_runner,
                &mut state.ctx,
                transition,
                transition.play_fade_in_ms,
            ) {
                Err(DecodeError::from(error))
            } else {
                active_runner.set_state(RunnerState::Playing);
                update_state(callback, &mut state.pumping, PlayerState::Playing);
                Ok(())
            }
        } else {
            active_runner.set_state(RunnerState::Playing);
            update_state(callback, &mut state.pumping, PlayerState::Playing);
            Ok(())
        }
    } else {
        Err(DecodeError::NoActivePipeline {
            operation: "play",
            reason: state.current_no_active_pipeline_reason(),
        })
    }
}
