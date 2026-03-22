use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use stellatune_audio_core::pipeline::context::PipelineContext;
use stellatune_audio_core::pipeline::stages::{StageRuntimeUpdateDispatchResult, StageTarget};

use crate::error::DecodeError;
use crate::pipeline::assembly::{PipelineAssembler, PipelineRuntime};
use crate::pipeline::runtime::dsp::control::{MASTER_GAIN_STAGE_KEY, MasterGainControl};
use crate::pipeline::runtime::runner::PipelineRunner;
use crate::pipeline::runtime::sink_session::SinkSession;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::reconfigure_active;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn apply_master_gain_level_to_runner(
    runner: &mut PipelineRunner,
    ctx: &mut PipelineContext,
    level: f32,
    ramp_ms: u32,
) -> Result<(), DecodeError> {
    let update = MasterGainControl::new(level, ramp_ms);
    runner.apply_stage_runtime_update_to(
        &StageTarget::transform(MASTER_GAIN_STAGE_KEY),
        Arc::new(update),
        None,
        ctx,
    )?;
    Ok(())
}

pub(crate) fn replay_persisted_stage_runtime_updates_to_runner(
    stage_runtime_updates: &HashMap<StageTarget, Arc<dyn Any + Send + Sync>>,
    runner: &mut PipelineRunner,
    sink_session: Option<&SinkSession>,
    ctx: &mut PipelineContext,
) -> Result<(), DecodeError> {
    let mut entries = stage_runtime_updates.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (target, update) in entries {
        match runner.apply_stage_runtime_update_to(target, Arc::clone(update), sink_session, ctx) {
            Ok(StageRuntimeUpdateDispatchResult::Applied) => {},
            Ok(StageRuntimeUpdateDispatchResult::StageNotFound) => {},
            Err(error) => {
                return Err(DecodeError::PersistedStageRuntimeUpdateApplyFailed {
                    target: target.clone(),
                    source: error,
                });
            },
        }
    }
    Ok(())
}

pub(crate) fn apply_policy_rebuild(
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> Result<(), DecodeError> {
    reconfigure_active::handle(assembler, callback, pipeline_runtime, state)
}
