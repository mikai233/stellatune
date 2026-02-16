use std::any::Any;

use crate::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use crate::pipeline::error::PipelineError;

use super::StageStatus;

pub trait TransformStage: Send {
    fn stage_key(&self) -> Option<&str> {
        None
    }

    fn apply_control(
        &mut self,
        _control: &dyn Any,
        _ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        Ok(false)
    }

    fn prepare(
        &mut self,
        spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError>;

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn process(&mut self, block: &mut AudioBlock, ctx: &mut PipelineContext) -> StageStatus;

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
