use crossbeam_channel::Sender;

use crate::worker::decode_loop::command_handler::control_apply;
use crate::worker::decode_loop::loop_state::DecodeLoopState;

pub(crate) fn handle(
    level: f32,
    resp_tx: Sender<Result<(), String>>,
    state: &mut DecodeLoopState,
) -> bool {
    state.set_master_gain_level(level);
    let result = control_apply::apply_master_gain_hot(state);
    let _ = resp_tx.send(result);
    false
}
