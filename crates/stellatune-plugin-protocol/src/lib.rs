#![allow(unexpected_cfgs)]

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
pub use stellatune_core::RequestId;
use stellatune_core::{
    AudioBackend, Command, ControlCommand, ControlScope, LfeMode, LibraryCommand,
    LibraryControlCommand, OutputSinkRoute, PlayerControlCommand, TrackRef,
};

#[cfg_attr(feature = "frb", flutter_rust_bridge::frb(non_opaque))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostControlAck {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMetadataVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl PluginMetadataVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

#[cfg_attr(feature = "frb", flutter_rust_bridge::frb(ignore))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub api_version: u32,
    pub version: PluginMetadataVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<Map<String, Value>>,
}

impl PluginMetadata {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        api_version: u32,
        version: PluginMetadataVersion,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            api_version,
            version,
            info: None,
        }
    }

    pub fn with_info(mut self, info: Option<Map<String, Value>>) -> Self {
        self.info = info;
        self
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryListTracksQuery {
    #[serde(default)]
    pub folder: String,
    #[serde(default = "default_true")]
    pub recursive: bool,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_list_tracks_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

impl Default for LibraryListTracksQuery {
    fn default() -> Self {
        Self {
            folder: String::new(),
            recursive: true,
            query: String::new(),
            limit: 5000,
            offset: 0,
        }
    }
}

impl LibraryListTracksQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn folder(mut self, folder: impl Into<String>) -> Self {
        self.folder = folder.into();
        self
    }

    pub fn recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibrarySearchQuery {
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

impl Default for LibrarySearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            limit: 200,
            offset: 0,
        }
    }
}

impl LibrarySearchQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryListPlaylistTracksQuery {
    pub playlist_id: i64,
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_list_tracks_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

impl LibraryListPlaylistTracksQuery {
    pub fn new(playlist_id: i64) -> Self {
        Self {
            playlist_id,
            query: String::new(),
            limit: 5000,
            offset: 0,
        }
    }

    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = offset;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum PlayerControl {
    SwitchTrackRef {
        track: TrackRef,
        #[serde(default)]
        lazy: bool,
    },
    Play,
    Pause,
    Stop,
    Shutdown,
    RefreshDevices,
    SeekMs {
        position_ms: u64,
    },
    SetVolume {
        volume: f32,
    },
    SetLfeMode {
        mode: LfeMode,
    },
    SetOutputDevice {
        backend: AudioBackend,
        #[serde(default, skip_serializing_if = "Option::is_none")]
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
        #[serde(default)]
        position_ms: u64,
    },
    PreloadTrackRef {
        track: TrackRef,
        #[serde(default)]
        position_ms: u64,
    },
}

impl PlayerControl {
    pub fn switch_track_ref(track: TrackRef) -> Self {
        Self::SwitchTrackRef { track, lazy: false }
    }

    pub fn switch_track_ref_lazy(track: TrackRef) -> Self {
        Self::SwitchTrackRef { track, lazy: true }
    }

    pub fn play() -> Self {
        Self::Play
    }

    pub fn pause() -> Self {
        Self::Pause
    }

    pub fn stop() -> Self {
        Self::Stop
    }

    pub fn shutdown() -> Self {
        Self::Shutdown
    }

    pub fn refresh_devices() -> Self {
        Self::RefreshDevices
    }

    pub fn seek_ms(position_ms: u64) -> Self {
        Self::SeekMs { position_ms }
    }

    pub fn set_volume(volume: f32) -> Self {
        Self::SetVolume { volume }
    }

    pub fn set_lfe_mode(mode: LfeMode) -> Self {
        Self::SetLfeMode { mode }
    }

    pub fn set_output_device(backend: AudioBackend, device_id: Option<String>) -> Self {
        Self::SetOutputDevice { backend, device_id }
    }

    pub fn set_output_options(
        match_track_sample_rate: bool,
        gapless_playback: bool,
        seek_track_fade: bool,
    ) -> Self {
        Self::SetOutputOptions {
            match_track_sample_rate,
            gapless_playback,
            seek_track_fade,
        }
    }

    pub fn set_output_sink_route(route: OutputSinkRoute) -> Self {
        Self::SetOutputSinkRoute { route }
    }

    pub fn clear_output_sink_route() -> Self {
        Self::ClearOutputSinkRoute
    }

    pub fn preload_track(path: impl Into<String>) -> Self {
        Self::PreloadTrack {
            path: path.into(),
            position_ms: 0,
        }
    }

    pub fn preload_track_at(path: impl Into<String>, position_ms: u64) -> Self {
        Self::PreloadTrack {
            path: path.into(),
            position_ms,
        }
    }

    pub fn preload_track_ref(track: TrackRef) -> Self {
        Self::PreloadTrackRef {
            track,
            position_ms: 0,
        }
    }

    pub fn preload_track_ref_at(track: TrackRef, position_ms: u64) -> Self {
        Self::PreloadTrackRef { track, position_ms }
    }

    pub fn command(&self) -> PlayerControlCommand {
        match self {
            Self::SwitchTrackRef { .. } => PlayerControlCommand::SwitchTrackRef,
            Self::Play => PlayerControlCommand::Play,
            Self::Pause => PlayerControlCommand::Pause,
            Self::Stop => PlayerControlCommand::Stop,
            Self::Shutdown => PlayerControlCommand::Shutdown,
            Self::RefreshDevices => PlayerControlCommand::RefreshDevices,
            Self::SeekMs { .. } => PlayerControlCommand::SeekMs,
            Self::SetVolume { .. } => PlayerControlCommand::SetVolume,
            Self::SetLfeMode { .. } => PlayerControlCommand::SetLfeMode,
            Self::SetOutputDevice { .. } => PlayerControlCommand::SetOutputDevice,
            Self::SetOutputOptions { .. } => PlayerControlCommand::SetOutputOptions,
            Self::SetOutputSinkRoute { .. } => PlayerControlCommand::SetOutputSinkRoute,
            Self::ClearOutputSinkRoute => PlayerControlCommand::ClearOutputSinkRoute,
            Self::PreloadTrack { .. } => PlayerControlCommand::PreloadTrack,
            Self::PreloadTrackRef { .. } => PlayerControlCommand::PreloadTrackRef,
        }
    }

    pub fn to_command(&self) -> Command {
        match self {
            Self::SwitchTrackRef { track, lazy } => Command::SwitchTrackRef {
                track: track.clone(),
                lazy: *lazy,
            },
            Self::Play => Command::Play,
            Self::Pause => Command::Pause,
            Self::Stop => Command::Stop,
            Self::Shutdown => Command::Shutdown,
            Self::RefreshDevices => Command::RefreshDevices,
            Self::SeekMs { position_ms } => Command::SeekMs {
                position_ms: *position_ms,
            },
            Self::SetVolume { volume } => Command::SetVolume { volume: *volume },
            Self::SetLfeMode { mode } => Command::SetLfeMode { mode: *mode },
            Self::SetOutputDevice { backend, device_id } => Command::SetOutputDevice {
                backend: *backend,
                device_id: device_id.clone(),
            },
            Self::SetOutputOptions {
                match_track_sample_rate,
                gapless_playback,
                seek_track_fade,
            } => Command::SetOutputOptions {
                match_track_sample_rate: *match_track_sample_rate,
                gapless_playback: *gapless_playback,
                seek_track_fade: *seek_track_fade,
            },
            Self::SetOutputSinkRoute { route } => Command::SetOutputSinkRoute {
                route: route.clone(),
            },
            Self::ClearOutputSinkRoute => Command::ClearOutputSinkRoute,
            Self::PreloadTrack { path, position_ms } => Command::PreloadTrack {
                path: path.clone(),
                position_ms: *position_ms,
            },
            Self::PreloadTrackRef { track, position_ms } => Command::PreloadTrackRef {
                track: track.clone(),
                position_ms: *position_ms,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum LibraryControl {
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
    ScanAll,
    ScanAllForce,
    ListRoots,
    ListFolders,
    ListExcludedFolders,
    ListTracks {
        #[serde(flatten, default)]
        query: LibraryListTracksQuery,
    },
    Search {
        #[serde(flatten, default)]
        query: LibrarySearchQuery,
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
        #[serde(flatten)]
        query: LibraryListPlaylistTracksQuery,
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
        #[serde(default)]
        liked: bool,
    },
    Shutdown,
}

impl LibraryControl {
    pub fn add_root(path: impl Into<String>) -> Self {
        Self::AddRoot { path: path.into() }
    }

    pub fn remove_root(path: impl Into<String>) -> Self {
        Self::RemoveRoot { path: path.into() }
    }

    pub fn delete_folder(path: impl Into<String>) -> Self {
        Self::DeleteFolder { path: path.into() }
    }

    pub fn restore_folder(path: impl Into<String>) -> Self {
        Self::RestoreFolder { path: path.into() }
    }

    pub fn scan_all() -> Self {
        Self::ScanAll
    }

    pub fn scan_all_force() -> Self {
        Self::ScanAllForce
    }

    pub fn list_roots() -> Self {
        Self::ListRoots
    }

    pub fn list_folders() -> Self {
        Self::ListFolders
    }

    pub fn list_excluded_folders() -> Self {
        Self::ListExcludedFolders
    }

    pub fn list_tracks(query: LibraryListTracksQuery) -> Self {
        Self::ListTracks { query }
    }

    pub fn search(query: LibrarySearchQuery) -> Self {
        Self::Search { query }
    }

    pub fn list_playlists() -> Self {
        Self::ListPlaylists
    }

    pub fn create_playlist(name: impl Into<String>) -> Self {
        Self::CreatePlaylist { name: name.into() }
    }

    pub fn rename_playlist(id: i64, name: impl Into<String>) -> Self {
        Self::RenamePlaylist {
            id,
            name: name.into(),
        }
    }

    pub fn delete_playlist(id: i64) -> Self {
        Self::DeletePlaylist { id }
    }

    pub fn list_playlist_tracks(query: LibraryListPlaylistTracksQuery) -> Self {
        Self::ListPlaylistTracks { query }
    }

    pub fn add_track_to_playlist(playlist_id: i64, track_id: i64) -> Self {
        Self::AddTrackToPlaylist {
            playlist_id,
            track_id,
        }
    }

    pub fn add_tracks_to_playlist(playlist_id: i64, track_ids: Vec<i64>) -> Self {
        Self::AddTracksToPlaylist {
            playlist_id,
            track_ids,
        }
    }

    pub fn remove_track_from_playlist(playlist_id: i64, track_id: i64) -> Self {
        Self::RemoveTrackFromPlaylist {
            playlist_id,
            track_id,
        }
    }

    pub fn remove_tracks_from_playlist(playlist_id: i64, track_ids: Vec<i64>) -> Self {
        Self::RemoveTracksFromPlaylist {
            playlist_id,
            track_ids,
        }
    }

    pub fn move_track_in_playlist(playlist_id: i64, track_id: i64, new_index: i64) -> Self {
        Self::MoveTrackInPlaylist {
            playlist_id,
            track_id,
            new_index,
        }
    }

    pub fn list_liked_track_ids() -> Self {
        Self::ListLikedTrackIds
    }

    pub fn set_track_liked(track_id: i64, liked: bool) -> Self {
        Self::SetTrackLiked { track_id, liked }
    }

    pub fn shutdown() -> Self {
        Self::Shutdown
    }

    pub fn command(&self) -> LibraryControlCommand {
        match self {
            Self::AddRoot { .. } => LibraryControlCommand::AddRoot,
            Self::RemoveRoot { .. } => LibraryControlCommand::RemoveRoot,
            Self::DeleteFolder { .. } => LibraryControlCommand::DeleteFolder,
            Self::RestoreFolder { .. } => LibraryControlCommand::RestoreFolder,
            Self::ScanAll => LibraryControlCommand::ScanAll,
            Self::ScanAllForce => LibraryControlCommand::ScanAllForce,
            Self::ListRoots => LibraryControlCommand::ListRoots,
            Self::ListFolders => LibraryControlCommand::ListFolders,
            Self::ListExcludedFolders => LibraryControlCommand::ListExcludedFolders,
            Self::ListTracks { .. } => LibraryControlCommand::ListTracks,
            Self::Search { .. } => LibraryControlCommand::Search,
            Self::ListPlaylists => LibraryControlCommand::ListPlaylists,
            Self::CreatePlaylist { .. } => LibraryControlCommand::CreatePlaylist,
            Self::RenamePlaylist { .. } => LibraryControlCommand::RenamePlaylist,
            Self::DeletePlaylist { .. } => LibraryControlCommand::DeletePlaylist,
            Self::ListPlaylistTracks { .. } => LibraryControlCommand::ListPlaylistTracks,
            Self::AddTrackToPlaylist { .. } => LibraryControlCommand::AddTrackToPlaylist,
            Self::AddTracksToPlaylist { .. } => LibraryControlCommand::AddTracksToPlaylist,
            Self::RemoveTrackFromPlaylist { .. } => LibraryControlCommand::RemoveTrackFromPlaylist,
            Self::RemoveTracksFromPlaylist { .. } => {
                LibraryControlCommand::RemoveTracksFromPlaylist
            }
            Self::MoveTrackInPlaylist { .. } => LibraryControlCommand::MoveTrackInPlaylist,
            Self::ListLikedTrackIds => LibraryControlCommand::ListLikedTrackIds,
            Self::SetTrackLiked { .. } => LibraryControlCommand::SetTrackLiked,
            Self::Shutdown => LibraryControlCommand::Shutdown,
        }
    }

    pub fn to_command(&self) -> LibraryCommand {
        match self {
            Self::AddRoot { path } => LibraryCommand::AddRoot { path: path.clone() },
            Self::RemoveRoot { path } => LibraryCommand::RemoveRoot { path: path.clone() },
            Self::DeleteFolder { path } => LibraryCommand::DeleteFolder { path: path.clone() },
            Self::RestoreFolder { path } => LibraryCommand::RestoreFolder { path: path.clone() },
            Self::ScanAll => LibraryCommand::ScanAll,
            Self::ScanAllForce => LibraryCommand::ScanAllForce,
            Self::ListRoots => LibraryCommand::ListRoots,
            Self::ListFolders => LibraryCommand::ListFolders,
            Self::ListExcludedFolders => LibraryCommand::ListExcludedFolders,
            Self::ListTracks { query } => LibraryCommand::ListTracks {
                folder: query.folder.clone(),
                recursive: query.recursive,
                query: query.query.clone(),
                limit: query.limit,
                offset: query.offset,
            },
            Self::Search { query } => LibraryCommand::Search {
                query: query.query.clone(),
                limit: query.limit,
                offset: query.offset,
            },
            Self::ListPlaylists => LibraryCommand::ListPlaylists,
            Self::CreatePlaylist { name } => LibraryCommand::CreatePlaylist { name: name.clone() },
            Self::RenamePlaylist { id, name } => LibraryCommand::RenamePlaylist {
                id: *id,
                name: name.clone(),
            },
            Self::DeletePlaylist { id } => LibraryCommand::DeletePlaylist { id: *id },
            Self::ListPlaylistTracks { query } => LibraryCommand::ListPlaylistTracks {
                playlist_id: query.playlist_id,
                query: query.query.clone(),
                limit: query.limit,
                offset: query.offset,
            },
            Self::AddTrackToPlaylist {
                playlist_id,
                track_id,
            } => LibraryCommand::AddTrackToPlaylist {
                playlist_id: *playlist_id,
                track_id: *track_id,
            },
            Self::AddTracksToPlaylist {
                playlist_id,
                track_ids,
            } => LibraryCommand::AddTracksToPlaylist {
                playlist_id: *playlist_id,
                track_ids: track_ids.clone(),
            },
            Self::RemoveTrackFromPlaylist {
                playlist_id,
                track_id,
            } => LibraryCommand::RemoveTrackFromPlaylist {
                playlist_id: *playlist_id,
                track_id: *track_id,
            },
            Self::RemoveTracksFromPlaylist {
                playlist_id,
                track_ids,
            } => LibraryCommand::RemoveTracksFromPlaylist {
                playlist_id: *playlist_id,
                track_ids: track_ids.clone(),
            },
            Self::MoveTrackInPlaylist {
                playlist_id,
                track_id,
                new_index,
            } => LibraryCommand::MoveTrackInPlaylist {
                playlist_id: *playlist_id,
                track_id: *track_id,
                new_index: *new_index,
            },
            Self::ListLikedTrackIds => LibraryCommand::ListLikedTrackIds,
            Self::SetTrackLiked { track_id, liked } => LibraryCommand::SetTrackLiked {
                track_id: *track_id,
                liked: *liked,
            },
            Self::Shutdown => LibraryCommand::Shutdown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum PluginControlRequest {
    Player {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<RequestId>,
        #[serde(flatten)]
        control: PlayerControl,
    },
    Library {
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<RequestId>,
        #[serde(flatten)]
        control: LibraryControl,
    },
}

impl PluginControlRequest {
    pub fn player(control: PlayerControl, request_id: Option<RequestId>) -> Self {
        Self::Player {
            request_id,
            control,
        }
    }

    pub fn library(control: LibraryControl, request_id: Option<RequestId>) -> Self {
        Self::Library {
            request_id,
            control,
        }
    }

    pub fn request_id(&self) -> Option<&RequestId> {
        match self {
            Self::Player { request_id, .. } | Self::Library { request_id, .. } => {
                request_id.as_ref()
            }
        }
    }

    pub fn scope(&self) -> ControlScope {
        match self {
            Self::Player { .. } => ControlScope::Player,
            Self::Library { .. } => ControlScope::Library,
        }
    }

    pub fn control_command(&self) -> ControlCommand {
        match self {
            Self::Player { control, .. } => control.command().into(),
            Self::Library { control, .. } => control.command().into(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_list_tracks_limit() -> i64 {
    5000
}

fn default_search_limit() -> i64 {
    200
}
