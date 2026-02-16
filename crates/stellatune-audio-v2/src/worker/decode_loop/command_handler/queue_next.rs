use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_audio_core::pipeline::context::InputRef;

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::worker::decode_loop::command_handler::open::prewarm_input;
use crate::worker::decode_loop::loop_state::DecodeLoopState;

pub(crate) fn handle(
    input: InputRef,
    resp_tx: Sender<Result<(), String>>,
    assembler: &Arc<dyn PipelineAssembler>,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
) -> bool {
    state.queued_next_input = Some(input.clone());
    state.prewarmed_next = None;
    let result = prewarm_input(input, assembler, pipeline_runtime, state).map(|prewarmed| {
        state.prewarmed_next = Some(prewarmed);
    });
    let _ = resp_tx.send(result.map(|_| ()));
    false
}
