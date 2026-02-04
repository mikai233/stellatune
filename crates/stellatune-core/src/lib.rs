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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    LoadTrack { path: String },
    Play,
    Pause,
    SetVolume { volume: f32 },
    Stop,
    Shutdown,
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
