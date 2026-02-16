use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::types::ResampleQuality;
use crate::worker::decode_loop::DecodeLoopEventCallback;
use crate::worker::decode_loop::command_handler::control_apply;
use crate::worker::decode_loop::loop_state::DecodeLoopState;

pub(crate) fn handle(
    quality: ResampleQuality,
    resp_tx: Sender<Result<(), String>>,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeLoopEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
) -> bool {
    state.set_resample_quality(quality);
    let result = control_apply::apply_policy_rebuild(assembler, callback, pipeline_runtime, state);
    let _ = resp_tx.send(result);
    false
}
