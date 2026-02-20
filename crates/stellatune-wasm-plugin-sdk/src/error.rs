use thiserror::Error;

pub type SdkResult<T> = Result<T, SdkError>;

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("invalid argument: {0}")]
    InvalidArg(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("denied: {0}")]
    Denied(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl SdkError {
    pub fn invalid_arg(message: impl Into<String>) -> Self {
        Self::InvalidArg(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::Io(message.into())
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::Timeout(message.into())
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    pub fn denied(message: impl Into<String>) -> Self {
        Self::Denied(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}
