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
