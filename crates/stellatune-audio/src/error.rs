//! Typed error enums for engine and decode worker operations.
//!
//! The crate uses [`EngineError`] as the top-level public error type for
//! engine APIs. Decode-specific failures are represented by [`DecodeError`]
//! and propagated through `EngineError::Decode`.

use thiserror::Error;

use lattice_actor::error::ActorCallError;
use stellatune_audio_core::pipeline::error::PipelineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoActivePipelineReason {
    NoTrackLoaded,
    SinkRecoveryInProgress {
        next_attempt: u32,
        last_error: Option<String>,
    },
    PipelineRebuildFailed {
        context: &'static str,
        error: String,
    },
    RunnerMissing,
}

impl std::fmt::Display for NoActivePipelineReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoTrackLoaded => f.write_str("no track is loaded"),
            Self::SinkRecoveryInProgress {
                next_attempt,
                last_error,
            } => {
                write!(
                    f,
                    "sink recovery in progress (retry {next_attempt} pending)"
                )?;
                if let Some(last_error) = last_error {
                    write!(f, ": {last_error}")?;
                }
                Ok(())
            },
            Self::PipelineRebuildFailed { context, error } => {
                write!(f, "pipeline rebuild failed during {context}: {error}")
            },
            Self::RunnerMissing => {
                f.write_str("an input is selected, but no playback runner is available")
            },
        }
    }
}

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
    #[error("no active pipeline to {operation}: {reason}")]
    NoActivePipeline {
        /// Operation name that required an active pipeline.
        operation: &'static str,
        /// Why an active pipeline is unavailable.
        reason: NoActivePipelineReason,
    },
    /// Sink recovery was requested without an active input.
    #[error("no active input for sink recovery")]
    NoActiveInputForRecovery,
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
    /// Playback actor could not be spawned.
    #[error("failed to spawn playback actor: {message}")]
    SpawnPlaybackActor {
        /// Lattice spawn failure text.
        message: String,
    },
    /// Playback actor mailbox could not accept a command before its deadline.
    #[error("playback actor mailbox is full while handling '{operation}'")]
    PlaybackMailboxFull {
        /// Operation name used for the actor call.
        operation: &'static str,
    },
    /// The requested operation is not admitted in the current playback state.
    #[error("playback operation '{operation}' is invalid in the current state")]
    InvalidPlaybackState {
        /// Operation rejected by behavior admission.
        operation: &'static str,
    },
    /// A playback command was rejected while plugin packages are changing.
    #[error("playback operation '{operation}' cannot run while a plugin change is in progress")]
    PluginChangeInProgress {
        /// Operation rejected during reconfiguration.
        operation: &'static str,
    },
    /// A Lattice playback handler failed unexpectedly.
    #[error("playback actor failed while handling '{operation}': {message}")]
    PlaybackActorFailed {
        /// Operation name used for the actor call.
        operation: &'static str,
        /// Handler failure text.
        message: String,
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
        err: ActorCallError,
    ) -> Self {
        match err {
            ActorCallError::MailboxFull => Self::PlaybackMailboxFull { operation },
            ActorCallError::UnhandledInCurrentState => Self::InvalidPlaybackState { operation },
            ActorCallError::InvalidTimeout | ActorCallError::DeadlineExceeded => {
                Self::ControlCommandTimedOut {
                    operation,
                    timeout_ms: timeout.as_millis(),
                }
            },
            ActorCallError::Handler(error) => Self::PlaybackActorFailed {
                operation,
                message: error.to_string(),
            },
            ActorCallError::MailboxClosed
            | ActorCallError::ActorPanicked
            | ActorCallError::LifecycleUnavailable { .. }
            | ActorCallError::ResponseDropped => Self::ControlActorExited { operation },
        }
    }
}
