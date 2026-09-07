use std::path::Path;

/// Metadata supplied by an optional application-owned local-source provider.
#[derive(Default)]
pub struct LocalFileMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    /// Embedded artwork bytes, ready for the library's normal cover cache.
    pub cover: Option<Vec<u8>>,
}

pub trait MetadataProvider: Send + Sync {
    /// Cheap lookup; called during directory enumeration and watch events.
    fn supports(&self, path: &Path) -> bool;
    /// Called on a blocking worker, never on an async runtime thread.
    fn inspect(&self, path: &Path) -> anyhow::Result<LocalFileMetadata>;
}
