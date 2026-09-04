//! Errors crossing audio pipeline and playback-control boundaries.
//!
//! Stage-local errors describe one operation.
//! [`PlaybackFailure`](crate::error::PlaybackFailure) adds the stage category,
//! stable item identity, generation, and retry policy needed by the runtime and
//! application layers.

use thiserror::Error;

use crate::{playback::PlaybackItemId, stage::StageId};

macro_rules! stage_error {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Error)]
        pub enum $name {
            /// The stage cannot make progress until more input or capacity is available.
            #[error("operation temporarily unavailable")]
            Pending,
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

impl FailureStage {
    /// Maps an internal stage label to its stable public category.
    ///
    /// Unknown labels map to [`Self::Runtime`]. Normalizer failures map to
    /// [`Self::Transform`], and recovery failures map to [`Self::Sink`].
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

/// The recovery action associated with a [`PlaybackFailure`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDisposition {
    /// The failure is terminal for the current playback attempt.
    Never,
    /// Recovery requires reopening and preparing the encoded source.
    ReopenSource,
    /// Recovery requires recreating the output sink.
    RebuildOutput,
    /// The same operation may be retried after a delay.
    Backoff,
}

/// A contextual failure emitted by the playback runtime.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("playback failed at {stage:?} ({code}): {message}")]
pub struct PlaybackFailure {
    /// The subsystem in which the failure originated.
    pub stage: FailureStage,
    /// The concrete registered stage, when one can be identified.
    pub stage_id: Option<StageId>,
    /// A stable, machine-readable error code.
    pub code: &'static str,
    /// A human-readable failure description.
    pub message: String,
    /// The recovery action recommended by the producing layer.
    pub retry: RetryDisposition,
    /// The affected playback item, when an item was active.
    pub item_id: Option<PlaybackItemId>,
    /// The playback generation in which the failure occurred.
    pub generation: u64,
}

impl PlaybackFailure {
    /// Creates a generic failure from an internal stage label and message.
    ///
    /// Source failures default to [`RetryDisposition::ReopenSource`], sink and
    /// recovery failures default to [`RetryDisposition::RebuildOutput`], and
    /// all other stages default to [`RetryDisposition::Never`].
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
    /// Creates a failed control result from an internal stage label and message.
    pub fn failed(stage: &'static str, message: impl Into<String>) -> Self {
        Self::Failed(PlaybackFailure::internal(stage, message))
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
