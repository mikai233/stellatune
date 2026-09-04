//! Stable identities for decoder, transform, and sink implementations.

/// A non-empty identifier for a registered audio pipeline stage.
///
/// Stage identifiers appear in deterministic ordering, diagnostics, persisted
/// configuration, and [`crate::error::PlaybackFailure`] context.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StageId(String);

impl StageId {
    /// Creates a stage identifier after trimming surrounding whitespace.
    ///
    /// # Errors
    ///
    /// Returns an error when the trimmed identifier is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err("stage id cannot be empty");
        }
        Ok(Self(value))
    }

    /// Returns the normalized string representation of this identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
