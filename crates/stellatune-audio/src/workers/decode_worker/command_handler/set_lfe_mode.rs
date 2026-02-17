use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::types::LfeMode;
use crate::workers::decode_worker::DecodeWorkerEventCallback;
use crate::workers::decode_worker::command_handler::control_apply;
use crate::workers::decode_worker::state::DecodeWorkerState;

pub(crate) fn handle(
    mode: LfeMode,
    resp_tx: Sender<Result<(), String>>,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    state.set_lfe_mode(mode);
    let result = control_apply::apply_policy_rebuild(assembler, callback, pipeline_runtime, state);
    let _ = resp_tx.send(result);
    false
}
