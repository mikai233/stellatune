use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("not found: {resource} `{id}`")]
    NotFound { resource: &'static str, id: String },
    #[error("conflict: {resource} `{id}`")]
    Conflict { resource: &'static str, id: String },
    #[error("unsupported: {message}")]
    Unsupported { message: String },
    #[error("{operation} failed: {details}")]
    Operation {
        operation: &'static str,
        details: String,
    },
    #[error("{operation} failed: {details}")]
    Aggregate {
        operation: &'static str,
        details: String,
    },
    #[error("io failed at `{path}`: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("json failed at `{path}`: {source}")]
    JsonAt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Walkdir(#[from] walkdir::Error),
    #[error(transparent)]
    Wasmtime(#[from] wasmtime::Error),
}

impl Error {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn not_found(resource: &'static str, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource,
            id: id.into(),
        }
    }

    pub fn conflict(resource: &'static str, id: impl Into<String>) -> Self {
        Self::Conflict {
            resource,
            id: id.into(),
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported {
            message: message.into(),
        }
    }

    pub fn operation(operation: &'static str, details: impl Into<String>) -> Self {
        Self::Operation {
            operation,
            details: details.into(),
        }
    }

    pub fn aggregate(operation: &'static str, errors: Vec<String>) -> Self {
        Self::Aggregate {
            operation,
            details: errors.join("; "),
        }
    }

    pub fn io_at(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::IoAt {
            path: path.into(),
            source,
        }
    }

    pub fn json_at(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::JsonAt {
            path: path.into(),
            source,
        }
    }
}

pub trait ErrorContext<T, E> {
    fn context(self, operation: &'static str) -> Result<T>;
    fn with_context<F>(self, operation: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T, E> for std::result::Result<T, E>
where
    E: std::fmt::Display,
{
    fn context(self, operation: &'static str) -> Result<T> {
        self.map_err(|error| Error::operation(operation, error.to_string()))
    }

    fn with_context<F>(self, operation: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|error| Error::operation("context", format!("{}: {}", operation(), error)))
    }
}

#[macro_export]
macro_rules! op_error {
    ($($arg:tt)*) => {
        $crate::error::Error::operation("runtime", format!($($arg)*))
    };
}
