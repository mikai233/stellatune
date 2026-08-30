use crate::pipeline::context::{
    AudioBlock, GainTransitionRequest, GaplessTrimSpec, MasterGainCurve, PipelineContext,
    StreamSpec,
};
use crate::pipeline::error::PipelineError;

use super::{Stage, StageFlow};

pub trait TransformStage: Stage {
    fn set_master_gain(
        &mut self,
        _level: f32,
        _ramp_ms: u32,
        _curve: Option<MasterGainCurve>,
    ) -> Result<bool, PipelineError> {
        Ok(false)
    }

    fn set_transition_gain(
        &mut self,
        _request: GainTransitionRequest,
    ) -> Result<bool, PipelineError> {
        Ok(false)
    }

    fn set_gapless_trim(
        &mut self,
        _spec: Option<GaplessTrimSpec>,
        _position_ms: i64,
    ) -> Result<bool, PipelineError> {
        Ok(false)
    }

    fn prepare(
        &mut self,
        spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError>;

    fn process(
        &mut self,
        block: &mut AudioBlock,
        ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError>;

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
