use thiserror::Error;

use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_runtime::thread_actor::CallError;

#[derive(Debug, Clone, Error)]
pub enum DecodeError {
    #[error("decode worker command queue full after {timeout_ms}ms")]
    CommandQueueFull { timeout_ms: u128 },
    #[error("decode worker command timed out after {timeout_ms}ms")]
    CommandTimedOut { timeout_ms: u128 },
    #[error("decode worker exited")]
    WorkerExited,
    #[error("decode worker shutdown timed out after {timeout_ms}ms")]
    ShutdownTimedOut { timeout_ms: u128 },
    #[error("decode worker thread panicked")]
    WorkerPanicked,
    #[error("no active pipeline to {operation}")]
    NoActivePipeline { operation: &'static str },
    #[error("no active input for sink recovery")]
    NoActiveInputForRecovery,
    #[error("transform stage not found for stage key: {stage_key}")]
    TransformStageNotFound { stage_key: String },
    #[error("failed to apply persisted stage control for '{stage_key}': {source}")]
    PersistedStageControlApplyFailed {
        stage_key: String,
        #[source]
        source: PipelineError,
    },
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("decode worker is not installed")]
    WorkerNotInstalled,
    #[error("decode worker already installed")]
    WorkerAlreadyInstalled,
    #[error("failed to spawn control actor: {source}")]
    SpawnControlActor {
        #[source]
        source: std::io::Error,
    },
    #[error("control actor command '{operation}' timed out after {timeout_ms}ms")]
    ControlCommandTimedOut {
        operation: &'static str,
        timeout_ms: u128,
    },
    #[error("control actor exited while handling '{operation}'")]
    ControlActorExited { operation: &'static str },
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
