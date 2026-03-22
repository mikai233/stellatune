pub mod decoder;
pub mod sink;
pub mod source;
pub mod transform;

use std::any::Any;
use std::fmt;

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
pub enum StageRuntimeUpdateResult {
    Applied,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StageTarget {
    Source,
    Decoder,
    Transform(String),
    Sink(String),
}

impl StageTarget {
    pub fn transform(key: impl Into<String>) -> Self {
        Self::Transform(key.into())
    }

    pub fn sink(key: impl Into<String>) -> Self {
        Self::Sink(key.into())
    }
}

impl fmt::Display for StageTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source => write!(f, "source"),
            Self::Decoder => write!(f, "decoder"),
            Self::Transform(key) => write!(f, "transform:{key}"),
            Self::Sink(key) => write!(f, "sink:{key}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageRuntimeUpdateDispatchResult {
    Applied,
    StageNotFound,
}

pub trait StageRuntimeUpdate: Any + Send + Sync {}

impl<T> StageRuntimeUpdate for T where T: Any + Send + Sync {}

pub fn downcast_runtime_update<T: Any>(update: &dyn StageRuntimeUpdate) -> Option<&T> {
    (update as &dyn Any).downcast_ref::<T>()
}

pub trait Stage: Send {
    fn key(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn apply_runtime_update(
        &mut self,
        _update: &dyn StageRuntimeUpdate,
        _ctx: &mut PipelineContext,
    ) -> Result<StageRuntimeUpdateResult, PipelineError> {
        Ok(StageRuntimeUpdateResult::Ignored)
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }
}
