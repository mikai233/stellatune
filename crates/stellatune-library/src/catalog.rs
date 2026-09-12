//! Typed browsing contract shared by native providers and the plugin boundary.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

mod local;
pub use local::LocalCatalog;

/// Local source identity and its optional audible sample interval.
#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(ignore)]
pub struct LocalTrackResource {
    pub path: String,
    pub segment: Option<stellatune_audio_core::segment::AudioSegment>,
    pub cover_key: i64,
    pub pcm_bits: Option<u32>,
    pub pcm_float: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    Track,
    Album,
    Artist,
    Folder,
    Playlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogSort {
    Default,
    Title,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaRef {
    /// Decimal host identity; plugin IDs never substitute for this namespace.
    pub source_instance_id: String,
    pub kind: MediaKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub reference: MediaRef,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub track_count: Option<i64>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub local_track_id: Option<i64>,
    #[serde(default)]
    pub local_cover_id: Option<i64>,
    #[serde(default)]
    pub is_segment: bool,
    #[serde(default)]
    pub audio: Option<CatalogAudioInfo>,
    #[serde(default)]
    pub local_path: Option<String>,
    #[serde(default)]
    pub album_ref: Option<MediaRef>,
    #[serde(default)]
    pub artist_refs: Vec<MediaRef>,
}

/// Cached source properties; browsing never opens or decodes audio files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAudioInfo {
    pub format: String,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub bits_per_sample: Option<u32>,
    pub floating_point: bool,
    pub channels: Option<u32>,
    pub bitrate: Option<stellatune_media_probe::BitrateInfo>,
    pub cue_path: Option<String>,
    pub start_frame: Option<i64>,
    pub end_frame: Option<i64>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub source_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub items: Vec<CatalogItem>,
    pub next_cursor: Option<String>,
    #[serde(default)]
    pub total: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogQuery {
    pub source_instance_id: String,
    pub kind: MediaKind,
    pub parent: Option<MediaRef>,
    pub search: String,
    pub sort: CatalogSort,
    pub cursor: Option<String>,
    pub limit: u32,
}

impl CatalogQuery {
    pub fn validate(&self) -> Result<()> {
        if self.source_instance_id.is_empty() || !(1..=200).contains(&self.limit) {
            bail!("invalid catalog source or page size (expected 1..200)");
        }
        if let Some(parent) = &self.parent {
            if parent.source_instance_id != self.source_instance_id || parent.id.is_empty() {
                bail!("catalog parent belongs to a different source or has an empty ID");
            }
        }
        if self.search.len() > 4096 || self.cursor.as_ref().is_some_and(|s| s.len() > 16384) {
            bail!("catalog query is too large");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySource {
    pub id: String,
    pub name: String,
    pub local: bool,
    pub available: bool,
    pub error: Option<String>,
    pub browse_kinds: Vec<MediaKind>,
    pub search_kinds: Vec<MediaKind>,
    pub sorts: Vec<CatalogSort>,
}

/// The plugin discovers account/server instances; configuration stays in its dataDir.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginLibraryInstance {
    pub instance_id: String,
    pub name: String,
    pub resolver_capability_id: String,
    pub browse_kinds: Vec<MediaKind>,
    pub search_kinds: Vec<MediaKind>,
    pub sorts: Vec<CatalogSort>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginLibraries {
    pub protocol_version: u32,
    pub instances: Vec<PluginLibraryInstance>,
}
