#![allow(unexpected_cfgs)]

use serde::{Deserialize, Serialize};

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Buffering,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LfeMode {
    #[default]
    Mute,
    MixToFront,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioBackend {
    Shared,
    WasapiExclusive,
    Asio,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioDevice {
    pub backend: AudioBackend,
    pub id: String,
    pub name: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackRef {
    /// Logical source id (e.g. `local`, `netease`, `onedrive`).
    pub source_id: String,
    /// Stable identifier inside the source.
    pub track_id: String,
    /// Opaque locator used by source/decoder implementations.
    pub locator: String,
}

impl TrackRef {
    pub fn new(source_id: String, track_id: String, locator: String) -> Self {
        Self {
            source_id,
            track_id,
            locator,
        }
    }

    pub fn for_local_path(path: String) -> Self {
        Self {
            source_id: "local".to_string(),
            track_id: path.clone(),
            locator: path,
        }
    }

    pub fn stable_key(&self) -> String {
        format!("{}:{}", self.source_id, self.track_id)
    }
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    LoadTrack {
        path: String,
    },
    LoadTrackRef {
        track: TrackRef,
    },
    Play,
    Pause,
    SeekMs {
        position_ms: u64,
    },
    SetVolume {
        volume: f32,
    },
    SetLfeMode {
        mode: LfeMode,
    },
    Stop,
    Shutdown,
    SetOutputDevice {
        backend: AudioBackend,
        device_id: Option<String>,
    },
    SetOutputOptions {
        match_track_sample_rate: bool,
        gapless_playback: bool,
        seek_track_fade: bool,
    },
    SetOutputSinkRoute {
        route: OutputSinkRoute,
    },
    ClearOutputSinkRoute,
    PreloadTrack {
        path: String,
        position_ms: u64,
    },
    PreloadTrackRef {
        track: TrackRef,
        position_ms: u64,
    },
    RefreshDevices,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DspChainItem {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DspTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceCatalogTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsProviderTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSinkTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSinkRoute {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
    pub target_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    StateChanged { state: PlayerState },
    Position { ms: i64 },
    TrackChanged { path: String },
    PlaybackEnded { path: String },
    VolumeChanged { volume: f32 },
    Error { message: String },
    Log { message: String },
    OutputDevicesChanged { devices: Vec<AudioDevice> },
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackLite {
    pub id: i64,
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackDecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub metadata_json: Option<String>,
    pub decoder_plugin_id: Option<String>,
    pub decoder_type_id: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsQuery {
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub text: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsDoc {
    pub track_key: String,
    pub source: String,
    pub is_synced: bool,
    pub lines: Vec<LyricLine>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsSearchCandidate {
    pub candidate_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub source: String,
    pub is_synced: bool,
    pub preview: Option<String>,
    pub doc: LyricsDoc,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LyricsEvent {
    Loading { track_key: String },
    Ready { track_key: String, doc: LyricsDoc },
    Cursor { track_key: String, line_index: i64 },
    Empty { track_key: String },
    Error { track_key: String, message: String },
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LibraryCommand {
    AddRoot {
        path: String,
    },
    RemoveRoot {
        path: String,
    },
    DeleteFolder {
        path: String,
    },
    RestoreFolder {
        path: String,
    },
    ListExcludedFolders,
    ListRoots,
    ListFolders,
    ListTracks {
        folder: String,
        recursive: bool,
        query: String,
        limit: i64,
        offset: i64,
    },
    ScanAll,
    ScanAllForce,
    Search {
        query: String,
        limit: i64,
        offset: i64,
    },
    Shutdown,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LibraryEvent {
    Roots {
        paths: Vec<String>,
    },
    Folders {
        paths: Vec<String>,
    },
    ExcludedFolders {
        paths: Vec<String>,
    },
    Changed,
    Tracks {
        folder: String,
        recursive: bool,
        query: String,
        items: Vec<TrackLite>,
    },
    ScanProgress {
        scanned: i64,
        updated: i64,
        skipped: i64,
        errors: i64,
    },
    ScanFinished {
        duration_ms: i64,
        scanned: i64,
        updated: i64,
        skipped: i64,
        errors: i64,
    },
    SearchResult {
        query: String,
        items: Vec<TrackLite>,
    },
    Error {
        message: String,
    },
    Log {
        message: String,
    },
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaSsdpDevice {
    pub usn: String,
    pub st: String,
    pub location: String,
    pub server: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaRenderer {
    pub usn: String,
    pub location: String,
    pub friendly_name: String,
    pub av_transport_control_url: Option<String>,
    pub av_transport_service_type: Option<String>,
    pub rendering_control_url: Option<String>,
    pub rendering_control_service_type: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaHttpServerInfo {
    pub listen_addr: String,
    pub base_url: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaTransportInfo {
    pub current_transport_state: String,
    pub current_transport_status: Option<String>,
    pub current_speed: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DlnaPositionInfo {
    pub rel_time_ms: u64,
    pub track_duration_ms: Option<u64>,
}
