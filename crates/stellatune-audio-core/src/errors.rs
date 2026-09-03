use thiserror::Error;

use crate::{PlaybackItemId, StageId};

macro_rules! stage_error {
    ($name:ident) => {
        #[derive(Debug, Error)]
        pub enum $name {
            #[error("operation temporarily unavailable")]
            Pending,
            #[error("operation is unsupported")]
            Unsupported,
            #[error("I/O failed: {0}")]
            Io(#[from] std::io::Error),
            #[error("operation failed: {message}")]
            Failed { message: String },
        }
    };
}

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("operation was cancelled")]
    Cancelled,
    #[error("operation temporarily unavailable")]
    Pending,
    #[error("operation is unsupported")]
    Unsupported,
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("operation failed: {message}")]
    Failed { message: String },
}

stage_error!(DecodeError);
stage_error!(TransformError);
stage_error!(SinkError);

#[derive(Debug, Error)]
pub enum FactoryError {
    #[error("factory configuration is invalid: {message}")]
    InvalidConfiguration { message: String },
    #[error("factory could not create a stage: {message}")]
    CreateFailed { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStage {
    Source,
    Decoder,
    Transform,
    Sink,
    Planner,
    Runtime,
}

impl FailureStage {
    pub fn from_internal_name(stage: &'static str) -> Self {
        match stage {
            "source" => Self::Source,
            "decoder" => Self::Decoder,
            "transform" | "normalizer" => Self::Transform,
            "sink" | "recovery" => Self::Sink,
            "planner" => Self::Planner,
            _ => Self::Runtime,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDisposition {
    Never,
    ReopenSource,
    RebuildOutput,
    Backoff,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("playback failed at {stage:?} ({code}): {message}")]
pub struct PlaybackFailure {
    pub stage: FailureStage,
    pub stage_id: Option<StageId>,
    pub code: &'static str,
    pub message: String,
    pub retry: RetryDisposition,
    pub item_id: Option<PlaybackItemId>,
    pub generation: u64,
}

impl PlaybackFailure {
    pub fn internal(stage: &'static str, message: impl Into<String>) -> Self {
        let stage = FailureStage::from_internal_name(stage);
        let retry = match stage {
            FailureStage::Source => RetryDisposition::ReopenSource,
            FailureStage::Sink => RetryDisposition::RebuildOutput,
            _ => RetryDisposition::Never,
        };
        Self {
            stage,
            stage_id: None,
            code: "stage_failed",
            message: message.into(),
            retry,
            item_id: None,
            generation: 0,
        }
    }

    pub fn with_context(mut self, item_id: Option<PlaybackItemId>, generation: u64) -> Self {
        self.item_id = item_id;
        self.generation = generation;
        self
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PlaybackControlError {
    #[error("playback runtime is closed")]
    Closed,
    #[error("playback command `{operation}` timed out")]
    CommandTimeout { operation: &'static str },
    #[error("command is invalid in the current playback state")]
    InvalidState,
    #[error("command is unsupported")]
    Unsupported,
    #[error(transparent)]
    Failed(PlaybackFailure),
}

impl PlaybackControlError {
    pub fn failed(stage: &'static str, message: impl Into<String>) -> Self {
        Self::Failed(PlaybackFailure::internal(stage, message))
    }

    pub fn with_context(self, item_id: Option<PlaybackItemId>, generation: u64) -> Self {
        match self {
            Self::Failed(failure) => Self::Failed(failure.with_context(item_id, generation)),
            other => other,
        }
    }
}
