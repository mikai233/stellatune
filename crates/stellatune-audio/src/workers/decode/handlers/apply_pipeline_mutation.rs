use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::error::DecodeError;
use crate::pipeline::assembly::{PipelineAssembler, PipelineMutation, PipelineRuntime};
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::control_apply;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn handle(
    mutation: PipelineMutation,
    resp_tx: Sender<Result<(), DecodeError>>,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    let result = (|| -> Result<(), DecodeError> {
        let next_blueprint =
            assembler.apply_pipeline_mutation(state.pinned_blueprint.as_deref(), mutation)?;
        state.pinned_blueprint = Some(next_blueprint);
        control_apply::apply_policy_rebuild(assembler, callback, pipeline_runtime, state)?;
        Ok(())
    })();
    let _ = resp_tx.send(result);
    false
}
