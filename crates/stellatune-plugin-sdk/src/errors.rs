use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkError {
    HostUnavailable,
    HostApiVersionMismatch {
        expected: u32,
        actual: u32,
    },
    HostCallbackUnavailable(&'static str),
    HostOperationFailed {
        operation: &'static str,
        code: i32,
        message: Option<String>,
    },
    InvalidArg(String),
    Serde(String),
    Io(String),
    Message(String),
}

impl SdkError {
    pub fn invalid_arg(msg: impl Into<String>) -> Self {
        Self::InvalidArg(msg.into())
    }

    pub fn msg(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SdkError::HostUnavailable => write!(f, "host vtable unavailable"),
            SdkError::HostApiVersionMismatch { expected, actual } => {
                write!(
                    f,
                    "host api version mismatch: expected={expected}, actual={actual}"
                )
            },
            SdkError::HostCallbackUnavailable(name) => {
                write!(f, "host callback `{name}` unavailable")
            },
            SdkError::HostOperationFailed {
                operation,
                code,
                message,
            } => {
                if let Some(message) = message
                    && !message.is_empty()
                {
                    return write!(f, "{operation} failed (code={code}): {message}");
                }
                write!(f, "{operation} failed (code={code})")
            },
            SdkError::InvalidArg(msg) => write!(f, "invalid arg: {msg}"),
            SdkError::Serde(msg) => write!(f, "serde error: {msg}"),
            SdkError::Io(msg) => write!(f, "io error: {msg}"),
            SdkError::Message(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for SdkError {}

impl From<String> for SdkError {
    fn from(value: String) -> Self {
        Self::Message(value)
    }
}

impl From<&str> for SdkError {
    fn from(value: &str) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<serde_json::Error> for SdkError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value.to_string())
    }
}

impl From<std::io::Error> for SdkError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

pub type SdkResult<T> = Result<T, SdkError>;
