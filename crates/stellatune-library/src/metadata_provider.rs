use std::path::Path;

/// Metadata supplied by an optional application-owned local-source provider.
#[derive(Default)]
pub struct LocalFileMetadata {
    pub audio: Option<stellatune_media_probe::AudioProperties>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub artists: Vec<String>,
    pub duration_ms: Option<i64>,
    /// Embedded artwork bytes, ready for the library's normal cover cache.
    pub cover: Option<Vec<u8>>,
}

pub trait MetadataProvider: Send + Sync {
    fn probe_audio(&self, path: &Path) -> stellatune_media_probe::ProbeResult {
        use stellatune_media_probe::{ProbeResult, ProbeStatus};
        match self.inspect_audio(path) {
            Ok(properties) => ProbeResult {
                status: if properties.is_some() {
                    ProbeStatus::Ready
                } else {
                    ProbeStatus::Unsupported
                },
                properties,
                bytes_read: 0,
            },
            Err(_) => ProbeResult {
                properties: None,
                status: ProbeStatus::IoError,
                bytes_read: 0,
            },
        }
    }
    fn inspect_audio(
        &self,
        path: &Path,
    ) -> anyhow::Result<Option<stellatune_media_probe::AudioProperties>> {
        self.inspect(path).map(|m| m.audio)
    }
    /// Cheap lookup; called during directory enumeration and watch events.
    fn supports(&self, path: &Path) -> bool;
    /// Called on a blocking worker, never on an async runtime thread.
    fn inspect(&self, path: &Path) -> anyhow::Result<LocalFileMetadata>;
}
