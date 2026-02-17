//! Typed error enums for engine and decode worker operations.
//!
//! The crate uses [`EngineError`] as the top-level public error type for
//! engine APIs. Decode-specific failures are represented by [`DecodeError`]
//! and propagated through `EngineError::Decode`.

use thiserror::Error;

use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_runtime::thread_actor::CallError;

/// Errors produced by decode worker command and runtime flows.
#[derive(Debug, Clone, Error)]
pub enum DecodeError {
    /// Decode command queue was full before enqueue completed.
    #[error("decode worker command queue full after {timeout_ms}ms")]
    CommandQueueFull {
        /// Timeout budget used when enqueueing the command.
        timeout_ms: u128,
    },
    /// Decode command response timed out.
    #[error("decode worker command timed out after {timeout_ms}ms")]
    CommandTimedOut {
        /// Timeout budget used while waiting for command completion.
        timeout_ms: u128,
    },
    /// Decode worker channel disconnected unexpectedly.
    #[error("decode worker exited")]
    WorkerExited,
    /// Shutdown acknowledgement timed out.
    #[error("decode worker shutdown timed out after {timeout_ms}ms")]
    ShutdownTimedOut {
        /// Timeout budget used while waiting for shutdown completion.
        timeout_ms: u128,
    },
    /// Decode worker thread panicked.
    #[error("decode worker thread panicked")]
    WorkerPanicked,
    /// Operation requires an active pipeline but none exists.
    #[error("no active pipeline to {operation}")]
    NoActivePipeline {
        /// Operation name that required an active pipeline.
        operation: &'static str,
    },
    /// Sink recovery was requested without an active input.
    #[error("no active input for sink recovery")]
    NoActiveInputForRecovery,
    /// Target transform stage key does not exist.
    #[error("transform stage not found for stage key: {stage_key}")]
    TransformStageNotFound {
        /// Missing stage key.
        stage_key: String,
    },
    /// Persisted stage control replay failed.
    #[error("failed to apply persisted stage control for '{stage_key}': {source}")]
    PersistedStageControlApplyFailed {
        /// Stage key that failed during replay.
        stage_key: String,
        /// Underlying pipeline error.
        #[source]
        source: PipelineError,
    },
    /// Wrapped pipeline-layer failure.
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
}

/// Errors produced by the engine control surface.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Engine command requires an installed decode worker.
    #[error("decode worker is not installed")]
    WorkerNotInstalled,
    /// Decode worker installation was attempted more than once.
    #[error("decode worker already installed")]
    WorkerAlreadyInstalled,
    /// Control actor thread could not be spawned.
    #[error("failed to spawn control actor: {source}")]
    SpawnControlActor {
        /// I/O error returned by thread spawn.
        #[source]
        source: std::io::Error,
    },
    /// Control actor call timed out.
    #[error("control actor command '{operation}' timed out after {timeout_ms}ms")]
    ControlCommandTimedOut {
        /// Operation name used for the actor call.
        operation: &'static str,
        /// Timeout budget used for the actor call.
        timeout_ms: u128,
    },
    /// Control actor exited before command completion.
    #[error("control actor exited while handling '{operation}'")]
    ControlActorExited {
        /// Operation name used for the actor call.
        operation: &'static str,
    },
    /// Wrapped decode worker error.
    #[error(transparent)]
    Decode(#[from] DecodeError),
}

impl EngineError {
    pub(crate) fn from_call_error(
        operation: &'static str,
        timeout: std::time::Duration,
        err: CallError,
    ) -> Self {
        match err {
            CallError::MailboxClosed | CallError::ActorStopped => {
                Self::ControlActorExited { operation }
            },
            CallError::Timeout => Self::ControlCommandTimedOut {
                operation,
                timeout_ms: timeout.as_millis(),
            },
        }
    }
}
