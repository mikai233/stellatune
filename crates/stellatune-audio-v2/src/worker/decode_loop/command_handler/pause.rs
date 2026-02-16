use crossbeam_channel::Sender;

use crate::types::{PauseBehavior, PlayerState};
use crate::worker::decode_loop::DecodeLoopEventCallback;
use crate::worker::decode_loop::command_handler::gain_transition;
use crate::worker::decode_loop::loop_state::DecodeLoopState;
use crate::worker::decode_loop::util::update_state;

pub(crate) fn handle(
    behavior: PauseBehavior,
    resp_tx: Sender<Result<(), String>>,
    callback: &DecodeLoopEventCallback,
    state: &mut DecodeLoopState,
) -> bool {
    let transition = state.gain_transition;
    let result = if let Some(active_runner) = state.runner.as_mut() {
        if state.state == PlayerState::Playing {
            let available_frames_hint = active_runner.playable_remaining_frames_hint();
            let _ = gain_transition::run_interrupt_fade_out(
                active_runner,
                &mut state.ctx,
                transition,
                transition.pause_fade_out_ms,
                available_frames_hint,
            );
        }
        match active_runner.pause(behavior, &mut state.ctx) {
            Ok(()) => {
                update_state(callback, &mut state.state, PlayerState::Paused);
                Ok(())
            },
            Err(error) => Err(error.to_string()),
        }
    } else {
        Err("no active pipeline to pause".to_string())
    };
    let _ = resp_tx.send(result);
    false
}
