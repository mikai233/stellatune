use crate::pipeline::context::{
    AudioBlock, GaplessTrimSpec, PipelineContext, SourceHandle, StreamSpec,
};
use crate::pipeline::error::PipelineError;

use super::StageStatus;

pub trait DecoderStage: Send {
    fn prepare(
        &mut self,
        source: &SourceHandle,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError>;

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn current_gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        None
    }

    fn estimated_remaining_frames(&self) -> Option<u64> {
        None
    }

    /// Returns optional runtime error detail after a fatal stage status.
    ///
    /// Implementations can expose richer context for diagnostics when
    /// `next_block` reports [`StageStatus::Fatal`].
    fn runtime_error_detail(&self) -> Option<&str> {
        None
    }

    fn next_block(&mut self, out: &mut AudioBlock, ctx: &mut PipelineContext) -> StageStatus;

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
