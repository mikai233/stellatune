use crate::config::engine::PlayerState;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;

pub(crate) fn handle(callback: &DecodeWorkerEventCallback, state: &mut DecodeWorkerState) {
    if let Some(active_runner) = state.runner.as_mut() {
        active_runner.stop(&mut state.sink_session, &mut state.ctx);
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
}
