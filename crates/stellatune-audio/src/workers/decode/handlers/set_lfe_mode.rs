use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::config::engine::LfeMode;
use crate::pipeline::assembly::{PipelineAssembler, PipelineRuntime};
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::control_apply;
use crate::workers::decode::state::DecodeWorkerState;

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
