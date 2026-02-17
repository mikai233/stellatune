use crossbeam_channel::Sender;

use crate::config::engine::{PauseBehavior, PlayerState};
use crate::error::DecodeError;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;

pub(crate) fn handle(
    behavior: PauseBehavior,
    resp_tx: Sender<Result<(), DecodeError>>,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> bool {
    let transition = state.gain_transition;
    let result = if let Some(active_runner) = state.runner.as_mut() {
        if state.state == PlayerState::Playing {
            let available_frames_hint = active_runner.playable_remaining_frames_hint();
            let _ = gain_transition::run_interrupt_fade_out(
                active_runner,
                &mut state.sink_session,
                &mut state.ctx,
                transition,
                transition.pause_fade_out_ms,
                available_frames_hint,
            );
        }
        match active_runner.pause(behavior, &mut state.sink_session, &mut state.ctx) {
            Ok(()) => {
                update_state(callback, &mut state.state, PlayerState::Paused);
                Ok(())
            },
            Err(error) => Err(DecodeError::from(error)),
        }
    } else {
        Err(DecodeError::NoActivePipeline { operation: "pause" })
    };
    let _ = resp_tx.send(result);
    false
}
