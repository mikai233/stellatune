use crate::pipeline::context::{
    AudioBlock, GaplessTrimSpec, PipelineContext, SourceHandle, StreamSpec,
};
use crate::pipeline::error::PipelineError;

use super::{Stage, StageFlow};

pub trait DecoderStage: Stage {
    fn prepare(
        &mut self,
        source: &SourceHandle,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError>;

    fn current_gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        None
    }

    fn estimated_remaining_frames(&self) -> Option<u64> {
        None
    }

    fn next_block(
        &mut self,
        out: &mut AudioBlock,
        ctx: &mut PipelineContext,
    ) -> Result<StageFlow, PipelineError>;

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError>;

    fn stop(&mut self, ctx: &mut PipelineContext);
}
