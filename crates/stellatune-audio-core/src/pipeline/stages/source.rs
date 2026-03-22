use crate::pipeline::context::{InputRef, PipelineContext, SourceHandle};
use crate::pipeline::error::PipelineError;

use super::Stage;

pub trait SourceStage: Stage {
    fn prepare(
        &mut self,
        input: &InputRef,
        ctx: &mut PipelineContext,
    ) -> Result<SourceHandle, PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
