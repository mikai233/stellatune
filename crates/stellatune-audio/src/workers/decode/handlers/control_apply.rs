use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use stellatune_audio_core::pipeline::context::PipelineContext;

use crate::error::DecodeError;
use crate::pipeline::assembly::{PipelineAssembler, PipelineRuntime};
use crate::pipeline::runtime::dsp::control::{MASTER_GAIN_STAGE_KEY, MasterGainControl};
use crate::pipeline::runtime::runner::PipelineRunner;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers::reconfigure_active;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) fn apply_master_gain_level_to_runner(
    runner: &mut PipelineRunner,
    ctx: &mut PipelineContext,
    level: f32,
    ramp_ms: u32,
) -> Result<(), DecodeError> {
    let control = MasterGainControl::new(level, ramp_ms);
    runner.apply_transform_control_to(MASTER_GAIN_STAGE_KEY, &control, ctx)?;
    Ok(())
}

pub(crate) fn replay_persisted_stage_controls_to_runner(
    stage_controls: &HashMap<String, Box<dyn Any + Send>>,
    runner: &mut PipelineRunner,
    ctx: &mut PipelineContext,
) -> Result<(), DecodeError> {
    let mut entries = stage_controls.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (stage_key, control) in entries {
        match runner.apply_transform_control_to(stage_key, control.as_ref(), ctx) {
            Ok(true) => {},
            Ok(false) => {},
            Err(error) => {
                return Err(DecodeError::PersistedStageControlApplyFailed {
                    stage_key: stage_key.to_string(),
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
