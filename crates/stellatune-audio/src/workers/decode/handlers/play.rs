use crossbeam_channel::Sender;

use crate::config::engine::PlayerState;
use crate::error::DecodeError;
use crate::pipeline::runtime::runner::RunnerState;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;

pub(crate) fn handle(
    resp_tx: Sender<Result<(), DecodeError>>,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> bool {
    let transition = state.gain_transition;
    let result = if let Some(active_runner) = state.runner.as_mut() {
        if state.state != PlayerState::Playing {
            if let Err(error) = gain_transition::request_fade_in_with_runner(
                active_runner,
                &mut state.ctx,
                transition,
                transition.play_fade_in_ms,
            ) {
                Err(DecodeError::from(error))
            } else {
                active_runner.set_state(RunnerState::Playing);
                update_state(callback, &mut state.state, PlayerState::Playing);
                Ok(())
            }
        } else {
            active_runner.set_state(RunnerState::Playing);
            update_state(callback, &mut state.state, PlayerState::Playing);
            Ok(())
        }
    } else {
        Err(DecodeError::NoActivePipeline { operation: "play" })
    };
    let _ = resp_tx.send(result);
    false
}
