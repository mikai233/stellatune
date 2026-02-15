use serde::{Deserialize, Serialize};

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
pub struct PlaylistLite {
    pub id: i64,
    pub name: String,
    pub system_key: Option<String>,
    pub track_count: i64,
    pub first_track_id: Option<i64>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LibraryEvent {
    Changed,
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
    Error {
        message: String,
    },
    Log {
        message: String,
    },
}
