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
    ListPlaylists,
    CreatePlaylist {
        name: String,
    },
    RenamePlaylist {
        id: i64,
        name: String,
    },
    DeletePlaylist {
        id: i64,
    },
    ListPlaylistTracks {
        playlist_id: i64,
        query: String,
        limit: i64,
        offset: i64,
    },
    AddTrackToPlaylist {
        playlist_id: i64,
        track_id: i64,
    },
    AddTracksToPlaylist {
        playlist_id: i64,
        track_ids: Vec<i64>,
    },
    RemoveTrackFromPlaylist {
        playlist_id: i64,
        track_id: i64,
    },
    RemoveTracksFromPlaylist {
        playlist_id: i64,
        track_ids: Vec<i64>,
    },
    MoveTrackInPlaylist {
        playlist_id: i64,
        track_id: i64,
        new_index: i64,
    },
    ListLikedTrackIds,
    SetTrackLiked {
        track_id: i64,
        liked: bool,
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
    Playlists {
        items: Vec<PlaylistLite>,
    },
    PlaylistTracks {
        playlist_id: i64,
        query: String,
        items: Vec<TrackLite>,
    },
    LikedTrackIds {
        track_ids: Vec<i64>,
    },
    Error {
        message: String,
    },
    Log {
        message: String,
    },
}
