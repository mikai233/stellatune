use crossbeam_channel::Sender;

use crate::runtime::runner::RunnerState;
use crate::types::PlayerState;
use crate::worker::decode_loop::DecodeLoopEventCallback;
use crate::worker::decode_loop::command_handler::gain_transition;
use crate::worker::decode_loop::loop_state::DecodeLoopState;
use crate::worker::decode_loop::util::update_state;

pub(crate) fn handle(
    resp_tx: Sender<Result<(), String>>,
    callback: &DecodeLoopEventCallback,
    state: &mut DecodeLoopState,
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
                Err(error.to_string())
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
        Err("no active pipeline to play".to_string())
    };
    let _ = resp_tx.send(result);
    false
}
