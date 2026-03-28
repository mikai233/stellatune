use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::error::DecodeError;
use crate::pipeline::assembly::{PipelineAssembler, PipelineMutation, PipelineRuntime};
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::control_apply;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn handle(
    mutations: Vec<PipelineMutation>,
    resp_tx: Sender<Result<(), DecodeError>>,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    let result = (|| -> Result<(), DecodeError> {
        let mut next_blueprint = state.pinned_blueprint.as_deref();
        for mutation in mutations {
            let updated = assembler.apply_pipeline_mutation(next_blueprint, mutation)?;
            state.pinned_blueprint = Some(updated);
            next_blueprint = state.pinned_blueprint.as_deref();
        }
        control_apply::apply_policy_rebuild(assembler, callback, pipeline_runtime, state)?;
        Ok(())
    })();
    let _ = resp_tx.send(result);
    false
}
