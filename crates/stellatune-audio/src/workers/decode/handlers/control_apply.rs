use stellatune_audio_core::pipeline::context::PipelineContext;

use crate::error::DecodeError;
use crate::pipeline::runtime::runner::PipelineRunner;

pub(crate) fn apply_master_gain_level_to_runner(
    runner: &mut PipelineRunner,
    _ctx: &mut PipelineContext,
    level: f32,
    ramp_ms: u32,
) -> Result<(), DecodeError> {
    runner.set_master_gain(level, ramp_ms, None)?;
    Ok(())
}
