use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::assembly::{PipelineAssembler, PipelineMutation, PipelineRuntime};
use crate::workers::decode_worker::DecodeWorkerEventCallback;
use crate::workers::decode_worker::command_handler::control_apply;
use crate::workers::decode_worker::state::DecodeWorkerState;

pub(crate) fn handle(
    mutation: PipelineMutation,
    resp_tx: Sender<Result<(), String>>,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    let result = (|| {
        pipeline_runtime
            .apply_pipeline_mutation(mutation)
            .map_err(|e| e.to_string())?;
        control_apply::apply_policy_rebuild(assembler, callback, pipeline_runtime, state)?;
        Ok(())
    })();
    let _ = resp_tx.send(result);
    false
}
