use crossbeam_channel::Sender;

use crate::assembly::PipelineRuntime;
use crate::types::DspChainSpec;
use crate::worker::decode_loop::command_handler::control_apply;

pub(crate) fn handle(
    spec: DspChainSpec,
    resp_tx: Sender<Result<(), String>>,
    pipeline_runtime: &mut dyn PipelineRuntime,
) -> bool {
    let result = control_apply::apply_dsp_chain_forward(spec, pipeline_runtime);
    let _ = resp_tx.send(result);
    false
}
