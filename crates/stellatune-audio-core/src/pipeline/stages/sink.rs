use crate::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use crate::pipeline::error::PipelineError;

use super::StageFlow;

pub trait SinkStage: Send {
    fn prepare(&mut self, spec: StreamSpec, ctx: &mut PipelineContext)
    -> Result<(), PipelineError>;

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn write(
        &mut self,
        block: &AudioBlock,
        ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError>;

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
