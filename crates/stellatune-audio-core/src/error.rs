//! Errors crossing audio pipeline and playback-control boundaries.
//!
//! Stage-local errors describe one operation.
//! [`PlaybackFailure`](crate::error::PlaybackFailure) adds the stage category,
//! stable item identity, generation, and failure category needed by the runtime and
//! application layers.

use thiserror::Error;

use crate::{playback::PlaybackItemId, stage::StageId};

macro_rules! stage_error {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Error)]
        pub enum $name {
            /// The stage does not implement the requested operation or format.
            #[error("operation is unsupported")]
            Unsupported,
            /// The stage encountered an underlying I/O error.
            #[error("I/O failed: {0}")]
            Io(#[from] std::io::Error),
            /// The stage failed with an implementation-specific explanation.
            #[error("operation failed: {message}")]
            Failed {
                /// A human-readable failure description.
                message: String,
            },
        }
    };
}

/// An error encountered while opening or consuming an encoded source.
#[derive(Debug, Error)]
pub enum SourceError {
    /// The source-opening operation was cancelled cooperatively.
    #[error("operation was cancelled")]
    Cancelled,
    /// The source is temporarily unable to make progress.
    #[error("operation temporarily unavailable")]
    Pending,
    /// The source does not support the requested operation.
    #[error("operation is unsupported")]
    Unsupported,
    /// The source encountered an underlying I/O error.
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// The source failed with an implementation-specific explanation.
    #[error("operation failed: {message}")]
    Failed {
        /// A human-readable failure description.
        message: String,
    },
}

stage_error!(DecodeError, "An error produced by a decoder stage.");
stage_error!(
    TransformError,
    "An error produced by a PCM transform stage."
);
stage_error!(SinkError, "An error produced by an audio output stage.");

/// An error encountered while constructing a pipeline stage.
#[derive(Debug, Error)]
pub enum FactoryError {
    /// The factory's static or runtime configuration is invalid.
    #[error("factory configuration is invalid: {message}")]
    InvalidConfiguration {
        /// A human-readable description of the invalid configuration.
        message: String,
    },
    /// The factory was configured correctly but could not create a stage.
    #[error("factory could not create a stage: {message}")]
    CreateFailed {
        /// A human-readable description of the creation failure.
        message: String,
    },
}

/// The pipeline subsystem in which playback failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureStage {
    /// Encoded source acquisition or reading.
    Source,
    /// Encoded audio decoding.
    Decoder,
    /// PCM transformation or format normalization.
    Transform,
    /// Audio output creation or writing.
    Sink,
    /// Pipeline stage selection and ordering.
    Planner,
    /// Playback scheduling, actor lifecycle, or another runtime invariant.
    Runtime,
}

/// A machine-readable reason, independent of runtime recovery policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCode {
    /// The requested operation or representation is unsupported.
    Unsupported,
    /// A source or stage encountered an I/O failure.
    Io,
    /// A factory received invalid configuration.
    InvalidConfiguration,
    /// A configured factory could not create a stage.
    CreateFailed,
    /// A stage reported an implementation-specific failure.
    StageFailed,
}

/// A contextual failure emitted by the playback runtime.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("playback failed at {stage:?} ({code:?}): {message}")]
pub struct PlaybackFailure {
    /// The subsystem in which the failure originated.
    pub stage: FailureStage,
    /// The concrete registered stage, when one can be identified.
    pub stage_id: Option<StageId>,
    /// A stable, machine-readable error code.
    pub code: FailureCode,
    /// A human-readable failure description.
    pub message: String,
    /// The affected playback item, when an item was active.
    pub item_id: Option<PlaybackItemId>,
    /// The playback generation in which the failure occurred.
    pub generation: u64,
}

impl PlaybackFailure {
    /// Creates a failure without making runtime recovery decisions.
    pub fn new(
        stage: FailureStage,
        code: FailureCode,
        stage_id: Option<StageId>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            stage_id,
            code,
            message: message.into(),
            item_id: None,
            generation: 0,
        }
    }

    /// Attaches playback-item and generation context to this failure.
    pub fn with_context(mut self, item_id: Option<PlaybackItemId>, generation: u64) -> Self {
        self.item_id = item_id;
        self.generation = generation;
        self
    }
}

/// An error returned to a caller of the playback control API.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PlaybackControlError {
    /// The playback actor is no longer accepting commands.
    #[error("playback runtime is closed")]
    Closed,
    /// A command did not complete before its configured deadline.
    #[error("playback command `{operation}` timed out")]
    CommandTimeout {
        /// The stable name of the timed-out control operation.
        operation: &'static str,
    },
    /// The command is not admitted by the current playback state.
    #[error("command is invalid in the current playback state")]
    InvalidState,
    /// The requested control operation is not supported.
    #[error("command is unsupported")]
    Unsupported,
    /// Playback failed while executing the command.
    #[error(transparent)]
    Failed(PlaybackFailure),
}

impl PlaybackControlError {
    /// Creates a failed control result for a typed subsystem and message.
    pub fn failed(stage: FailureStage, message: impl Into<String>) -> Self {
        Self::Failed(PlaybackFailure::new(
            stage,
            FailureCode::StageFailed,
            None,
            message,
        ))
    }

    /// Adds item and generation context when this is a [`Self::Failed`] error.
    ///
    /// Other control errors are returned unchanged.
    pub fn with_context(self, item_id: Option<PlaybackItemId>, generation: u64) -> Self {
        match self {
            Self::Failed(failure) => Self::Failed(failure.with_context(item_id, generation)),
            other => other,
        }
    }
}

macro_rules! stage_failure_conversion {
    ($error:ident, $stage:expr, $method:ident) => {
        impl PlaybackControlError {
            /// Preserves a stage failure category and concrete implementation identity.
            pub fn $method(error: $error, id: StageId) -> Self {
                let code = match &error {
                    $error::Unsupported => FailureCode::Unsupported,
                    $error::Io(_) => FailureCode::Io,
                    $error::Failed { .. } => FailureCode::StageFailed,
                };
                Self::Failed(PlaybackFailure::new(
                    $stage,
                    code,
                    Some(id),
                    error.to_string(),
                ))
            }
        }
    };
}
stage_failure_conversion!(DecodeError, FailureStage::Decoder, decoder);
stage_failure_conversion!(TransformError, FailureStage::Transform, transform);
stage_failure_conversion!(SinkError, FailureStage::Sink, sink);

impl PlaybackControlError {
    /// Preserves a factory error and identifies the factory that failed.
    pub fn factory(stage: FailureStage, id: StageId, error: FactoryError) -> Self {
        let code = match error {
            FactoryError::InvalidConfiguration { .. } => FailureCode::InvalidConfiguration,
            FactoryError::CreateFailed { .. } => FailureCode::CreateFailed,
        };
        Self::Failed(PlaybackFailure::new(
            stage,
            code,
            Some(id),
            error.to_string(),
        ))
    }
    /// Preserves source failures; cooperative cancellation remains a closed request.
    pub fn source(error: SourceError) -> Self {
        let code = match &error {
            SourceError::Cancelled => return Self::Closed,
            SourceError::Unsupported => FailureCode::Unsupported,
            SourceError::Io(_) => FailureCode::Io,
            _ => FailureCode::StageFailed,
        };
        Self::Failed(PlaybackFailure::new(
            FailureStage::Source,
            code,
            None,
            error.to_string(),
        ))
    }
}
