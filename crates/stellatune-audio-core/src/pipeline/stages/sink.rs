use crate::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use crate::pipeline::error::PipelineError;

use super::{Stage, StageFlow};

pub trait SinkStage: Stage {
    fn prepare(&mut self, spec: StreamSpec, ctx: &mut PipelineContext)
    -> Result<(), PipelineError>;

    fn write(
        &mut self,
        block: &AudioBlock,
        ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError>;

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
