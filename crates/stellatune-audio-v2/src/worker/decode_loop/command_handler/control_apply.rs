use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::runtime::runner::PipelineRunner;
use crate::runtime::transform::control::{MASTER_GAIN_STAGE_KEY, MasterGainControl};
use crate::types::DspChainSpec;
use crate::worker::decode_loop::DecodeLoopEventCallback;
use crate::worker::decode_loop::command_handler::reconfigure_active;
use crate::worker::decode_loop::loop_state::DecodeLoopState;

pub(super) fn apply_master_gain_hot(state: &mut DecodeLoopState) -> Result<(), String> {
    if let Some(active_runner) = state.runner.as_mut() {
        apply_master_gain_level_to_runner(active_runner, &mut state.ctx, state.master_gain_level)
    } else {
        Ok(())
    }
}

pub(crate) fn apply_master_gain_level_to_runner(
    runner: &mut PipelineRunner,
    ctx: &mut stellatune_audio_core::pipeline::context::PipelineContext,
    level: f32,
) -> Result<(), String> {
    let control = MasterGainControl::new(level);
    runner
        .apply_transform_control_to(MASTER_GAIN_STAGE_KEY, &control, ctx)
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub(crate) fn replay_persisted_stage_controls_to_runner(
    stage_controls: &HashMap<String, Box<dyn Any + Send>>,
    runner: &mut PipelineRunner,
    ctx: &mut stellatune_audio_core::pipeline::context::PipelineContext,
) -> Result<(), String> {
    let mut entries = stage_controls.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (stage_key, control) in entries {
        match runner.apply_transform_control_to(stage_key, control.as_ref(), ctx) {
            Ok(true) => {},
            Ok(false) => {},
            Err(error) => {
                return Err(format!(
                    "failed to apply persisted stage control for '{stage_key}': {error}"
                ));
            },
        }
    }
    Ok(())
}

pub(super) fn apply_dsp_chain_forward(
    spec: DspChainSpec,
    pipeline_runtime: &mut dyn PipelineRuntime,
) -> Result<(), String> {
    pipeline_runtime
        .apply_dsp_chain(spec)
        .map_err(|e| e.to_string())
}

pub(super) fn apply_policy_rebuild(
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeLoopEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
) -> Result<(), String> {
    reconfigure_active::handle(assembler, callback, pipeline_runtime, state)
}
