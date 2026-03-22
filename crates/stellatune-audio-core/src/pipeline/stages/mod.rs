pub mod decoder;
pub mod sink;
pub mod source;
pub mod transform;

use std::any::Any;

use crate::pipeline::context::PipelineContext;
use crate::pipeline::error::PipelineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageFlow {
    Continue,
    Eof,
}

impl StageFlow {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Eof)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageControlResult {
    Applied,
    Ignored,
}

pub trait Stage: Send {
    fn apply_control(
        &mut self,
        _control: &dyn Any,
        _ctx: &mut PipelineContext,
    ) -> Result<StageControlResult, PipelineError> {
        Ok(StageControlResult::Ignored)
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }
}
