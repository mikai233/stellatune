use serde::{Deserialize, Serialize};

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
