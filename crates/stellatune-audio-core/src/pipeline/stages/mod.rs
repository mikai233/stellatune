pub mod decoder;
pub mod sink;
pub mod source;
pub mod transform;

use crate::pipeline::context::PipelineContext;
use crate::pipeline::error::PipelineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageFlow {
    Continue,
    Eof,
}

pub trait Stage: Send {
    fn key(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn refresh_runtime_state(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }
}
