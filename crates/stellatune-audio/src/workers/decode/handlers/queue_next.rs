use std::sync::Arc;

use stellatune_audio_core::pipeline::context::InputRef;

use crate::error::DecodeError;
use crate::pipeline::assembly::PipelineFactory;
use crate::workers::decode::handlers::open::prewarm_input;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn handle(
    input: InputRef,
    factory: &Arc<dyn PipelineFactory>,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    let prewarmed = prewarm_input(input.clone(), factory, state)?;
    state.queued_next_input = Some(input);
    state.prewarmed_next = Some(prewarmed);
    Ok(())
}
