use crate::config::engine::{PlayerState, StopBehavior};
use crate::error::DecodeError;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn handle(
    behavior: StopBehavior,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    let transition = state.gain_transition;
    let mut stop_error: Option<DecodeError> = None;
    if let Some(active_runner) = state.runner.as_mut() {
        if state.pumping {
            let available_frames_hint = active_runner.playable_remaining_frames_hint();
            let _ = gain_transition::run_interrupt_fade_out(
                active_runner,
                &mut state.sink_session,
                &mut state.ctx,
                transition,
                transition.stop_fade_out_ms,
                available_frames_hint,
            );
        }
        if let Err(error) =
            active_runner.stop_with_behavior(behavior, &mut state.sink_session, &mut state.ctx)
        {
            stop_error = Some(DecodeError::from(error));
        }
    } else {
        state.sink_session.shutdown(false);
    }
    state.runner = None;
    state.reset_context();
    state.active_input = None;
    state.queued_next_input = None;
    state.prewarmed_next = None;
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;
    state.clear_pipeline_unavailable_reason();
    update_state(callback, &mut state.pumping, PlayerState::Stopped);
    callback(DecodeWorkerEvent::Position { position_ms: 0 });
    match stop_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
