use crossbeam_channel::Sender;

use crate::assembly::PipelineRuntime;
use crate::types::PlayerState;
use crate::worker::decode_loop::DecodeLoopEventCallback;
use crate::worker::decode_loop::loop_state::DecodeLoopState;
use crate::worker::decode_loop::util::update_state;

pub(crate) fn handle(
    ack_tx: Sender<()>,
    callback: &DecodeLoopEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
) -> bool {
    if let Some(active_runner) = state.runner.as_mut() {
        active_runner.stop(&mut state.ctx);
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
