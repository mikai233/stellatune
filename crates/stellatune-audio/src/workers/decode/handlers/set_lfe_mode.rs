use std::sync::Arc;

use crate::config::engine::LfeMode;
use crate::error::DecodeError;
use crate::pipeline::assembly::PipelineFactory;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn handle(
    mode: LfeMode,
    factory: &Arc<dyn PipelineFactory>,
    callback: &DecodeWorkerEventCallback,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    state.set_lfe_mode(mode);
    crate::workers::decode::handlers::reconfigure_active::handle(factory, callback, state)
}
