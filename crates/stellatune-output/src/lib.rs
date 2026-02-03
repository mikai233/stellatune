use thiserror::Error;

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("not implemented")]
    NotImplemented,
}

/// Output backend (planned: cpal wrapper).
///
/// The real implementation will run an audio callback thread where **no blocking**
/// and **minimal locking** are allowed. The callback should pull from a ring buffer.
pub trait OutputBackend: Send {
    fn start(&mut self) -> Result<(), OutputError> {
        Err(OutputError::NotImplemented)
    }

    fn stop(&mut self) -> Result<(), OutputError> {
        Err(OutputError::NotImplemented)
    }
}

/// Stub output backend used while the real cpal integration is pending.
pub struct StubOutput;

impl OutputBackend for StubOutput {}
