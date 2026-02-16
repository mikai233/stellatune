use crate::pipeline::context::{InputRef, PipelineContext, SourceHandle};
use crate::pipeline::error::PipelineError;

pub trait SourceStage: Send {
    fn prepare(
        &mut self,
        input: &InputRef,
        ctx: &mut PipelineContext,
    ) -> Result<SourceHandle, PipelineError>;

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
