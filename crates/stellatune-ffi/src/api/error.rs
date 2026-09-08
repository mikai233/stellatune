use std::hash::{Hash, Hasher};
use stellatune_audio_core::error::{FailureStage, PlaybackControlError};
use stellatune_backend_api::diagnostics::{LogRecord, shared};

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ErrorCategory {
    Cancelled,
    Unavailable,
    InvalidInput,
    NotFound,
    Timeout,
    Unsupported,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellatune_audio_core::error::{FailureCode, PlaybackFailure};
    #[test]
    fn command_and_event_share_typed_root_and_keep_full_diagnostics() {
        let failure = PlaybackControlError::Failed(PlaybackFailure::new(
            FailureStage::Sink,
            FailureCode::StageFailed,
            None,
            "hardware unavailable\nASIO details",
        ));
        let command = AppError::capture(
            "set_output",
            anyhow::Error::new(failure.clone()).context("select Realtek"),
        );
        let event = AppError::capture("playback_event", anyhow::Error::new(failure));
        assert_eq!(command.category, ErrorCategory::Unavailable);
        assert_eq!(command.fingerprint, event.fingerprint);
        assert_eq!(command.context, event.context);
        assert_ne!(command.diagnostic_id, event.diagnostic_id);
        let detail = shared().detail(&command.diagnostic_id).unwrap();
        assert!(detail.details.contains("select Realtek"));
        assert!(detail.details.contains("Rust backtrace:"));
        assert_eq!(
            AppError::capture("wrapper", anyhow::Error::new(command.clone())),
            command
        );
    }
    #[test]
    fn io_errors_keep_categories_without_message_parsing() {
        let error = AppError::capture(
            "read",
            std::io::Error::from(std::io::ErrorKind::NotFound).into(),
        );
        assert_eq!(error.category, ErrorCategory::NotFound);
        let closed = AppError::capture("pause", PlaybackControlError::Closed.into());
        assert_eq!(closed.category, ErrorCategory::Unavailable);
    }
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppError {
    pub category: ErrorCategory,
    pub operation: String,
    pub diagnostic_id: String,
    pub fingerprint: String,
    pub context: String,
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}: {} [{}]",
            self.category, self.operation, self.diagnostic_id
        )
    }
}
impl std::error::Error for AppError {}
impl AppError {
    #[flutter_rust_bridge::frb(ignore)]
    pub fn capture(operation: &str, error: anyhow::Error) -> Self {
        if let Some(error) = error.downcast_ref::<Self>() {
            return error.clone();
        }
        let mut category = ErrorCategory::Internal;
        let mut context = String::new();
        let mut root = error.root_cause().to_string();
        if let Some(control) = error
            .chain()
            .find_map(|e| e.downcast_ref::<PlaybackControlError>())
        {
            category = match control {
                PlaybackControlError::CommandTimeout { .. } => ErrorCategory::Timeout,
                PlaybackControlError::Unsupported => ErrorCategory::Unsupported,
                PlaybackControlError::InvalidState => ErrorCategory::InvalidInput,
                PlaybackControlError::Closed => ErrorCategory::Unavailable,
                PlaybackControlError::Failed(failure) => {
                    root = failure.message.clone();
                    context = format!(
                        "stage={:?}, item={:?}, generation={}",
                        failure.stage, failure.item_id, failure.generation
                    );
                    if failure.stage == FailureStage::Sink {
                        ErrorCategory::Unavailable
                    } else {
                        ErrorCategory::Internal
                    }
                },
            };
        } else if matches!(
            error.downcast_ref::<stellatune_audio_core::error::SourceError>(),
            Some(stellatune_audio_core::error::SourceError::Cancelled)
        ) {
            category = ErrorCategory::Cancelled;
        } else if let Some(io) = error.downcast_ref::<std::io::Error>() {
            category = match io.kind() {
                std::io::ErrorKind::NotFound => ErrorCategory::NotFound,
                std::io::ErrorKind::InvalidInput => ErrorCategory::InvalidInput,
                std::io::ErrorKind::TimedOut => ErrorCategory::Timeout,
                _ => ErrorCategory::Internal,
            };
        } else if operation.contains("output") || operation.contains("device") {
            category = ErrorCategory::Unavailable;
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        // Root cause is shared by command and event, even when the event adds context.
        root.hash(&mut hash);
        let fingerprint = format!("{:016x}", hash.finish());
        let mut record = LogRecord::new(
            "rust",
            if category == ErrorCategory::Cancelled {
                "DEBUG"
            } else {
                "ERROR"
            },
            operation,
            root,
            format!(
                "{error:#}\n{context}\nRust backtrace:\n{}",
                std::backtrace::Backtrace::force_capture()
            ),
        );
        record.fingerprint = Some(fingerprint.clone());
        let diagnostic_id = shared().record(record);
        Self {
            category,
            operation: operation.into(),
            diagnostic_id,
            fingerprint,
            context,
        }
    }
    #[flutter_rust_bridge::frb(ignore)]
    pub fn message(operation: &str, message: String) -> Self {
        Self::capture(operation, anyhow::anyhow!(message))
    }
}
