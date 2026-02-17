use crossbeam_channel::Sender;

use crate::config::engine::PlayerState;
use crate::pipeline::assembly::PipelineRuntime;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;

pub(crate) fn handle(
    ack_tx: Sender<()>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    if let Some(active_runner) = state.runner.as_mut() {
        active_runner.stop(&mut state.sink_session, &mut state.ctx);
    } else {
        state.sink_session.shutdown(false);
    }
    pipeline_runtime.reset();
    state.runner = None;
    state.reset_context();
    state.active_input = None;
    state.queued_next_input = None;
    state.prewarmed_next = None;
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;
    update_state(callback, &mut state.state, PlayerState::Stopped);
    let _ = ack_tx.send(());
    true
}
