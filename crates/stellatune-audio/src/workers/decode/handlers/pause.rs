use crate::config::engine::{PauseBehavior, PlayerState};
use crate::error::DecodeError;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;

pub(crate) fn handle(
    behavior: PauseBehavior,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    let transition = state.gain_transition;
    if let Some(active_runner) = state.runner.as_mut() {
        if state.pumping {
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
                update_state(callback, &mut state.pumping, PlayerState::Paused);
                Ok(())
            },
            Err(error) => Err(DecodeError::from(error)),
        }
    } else {
        Err(DecodeError::NoActivePipeline {
            operation: "pause",
            reason: state.current_no_active_pipeline_reason(),
        })
    }
}
