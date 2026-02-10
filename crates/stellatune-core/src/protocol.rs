use serde::{Deserialize, Serialize};

use crate::library::LibraryEvent;
use crate::playback::{Event, PlayerState};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(String);

impl RequestId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<String> for RequestId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RequestId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntimeKind {
    Notify,
    Control,
    ControlResult,
    ControlFinished,
}

impl PluginRuntimeKind {
    #[flutter_rust_bridge::frb(ignore)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Control => "control",
            Self::ControlResult => "control_result",
            Self::ControlFinished => "control_finished",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlScope {
    Player,
    Library,
}

impl ControlScope {
    #[flutter_rust_bridge::frb(ignore)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Library => "library",
        }
    }

    #[flutter_rust_bridge::frb(ignore)]
    pub fn from_str(v: &str) -> Option<Self> {
        match v {
            "player" => Some(Self::Player),
            "library" => Some(Self::Library),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerControlCommand {
    SwitchTrackRef,
    Play,
    Pause,
    Stop,
    Shutdown,
    RefreshDevices,
    SeekMs,
    SetVolume,
    SetLfeMode,
    SetOutputDevice,
    SetOutputOptions,
    SetOutputSinkRoute,
    ClearOutputSinkRoute,
    PreloadTrack,
    PreloadTrackRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlCommand {
    SwitchTrackRef,
    Play,
    Pause,
    Stop,
    Shutdown,
    RefreshDevices,
    SeekMs,
    SetVolume,
    SetLfeMode,
    SetOutputDevice,
    SetOutputOptions,
    SetOutputSinkRoute,
    ClearOutputSinkRoute,
    PreloadTrack,
    PreloadTrackRef,
    AddRoot,
    RemoveRoot,
    DeleteFolder,
    RestoreFolder,
    ScanAll,
    ScanAllForce,
    ListRoots,
    ListFolders,
    ListExcludedFolders,
    ListTracks,
    Search,
    ListPlaylists,
    CreatePlaylist,
    RenamePlaylist,
    DeletePlaylist,
    ListPlaylistTracks,
    AddTrackToPlaylist,
    AddTracksToPlaylist,
    RemoveTrackFromPlaylist,
    RemoveTracksFromPlaylist,
    MoveTrackInPlaylist,
    ListLikedTrackIds,
    SetTrackLiked,
}

impl ControlCommand {
    #[flutter_rust_bridge::frb(ignore)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SwitchTrackRef => "switch_track_ref",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Stop => "stop",
            Self::Shutdown => "shutdown",
            Self::RefreshDevices => "refresh_devices",
            Self::SeekMs => "seek_ms",
            Self::SetVolume => "set_volume",
            Self::SetLfeMode => "set_lfe_mode",
            Self::SetOutputDevice => "set_output_device",
            Self::SetOutputOptions => "set_output_options",
            Self::SetOutputSinkRoute => "set_output_sink_route",
            Self::ClearOutputSinkRoute => "clear_output_sink_route",
            Self::PreloadTrack => "preload_track",
            Self::PreloadTrackRef => "preload_track_ref",
            Self::AddRoot => "add_root",
            Self::RemoveRoot => "remove_root",
            Self::DeleteFolder => "delete_folder",
            Self::RestoreFolder => "restore_folder",
            Self::ScanAll => "scan_all",
            Self::ScanAllForce => "scan_all_force",
            Self::ListRoots => "list_roots",
            Self::ListFolders => "list_folders",
            Self::ListExcludedFolders => "list_excluded_folders",
            Self::ListTracks => "list_tracks",
            Self::Search => "search",
            Self::ListPlaylists => "list_playlists",
            Self::CreatePlaylist => "create_playlist",
            Self::RenamePlaylist => "rename_playlist",
            Self::DeletePlaylist => "delete_playlist",
            Self::ListPlaylistTracks => "list_playlist_tracks",
            Self::AddTrackToPlaylist => "add_track_to_playlist",
            Self::AddTracksToPlaylist => "add_tracks_to_playlist",
            Self::RemoveTrackFromPlaylist => "remove_track_from_playlist",
            Self::RemoveTracksFromPlaylist => "remove_tracks_from_playlist",
            Self::MoveTrackInPlaylist => "move_track_in_playlist",
            Self::ListLikedTrackIds => "list_liked_track_ids",
            Self::SetTrackLiked => "set_track_liked",
        }
    }
}

impl From<PlayerControlCommand> for ControlCommand {
    fn from(value: PlayerControlCommand) -> Self {
        match value {
            PlayerControlCommand::SwitchTrackRef => Self::SwitchTrackRef,
            PlayerControlCommand::Play => Self::Play,
            PlayerControlCommand::Pause => Self::Pause,
            PlayerControlCommand::Stop => Self::Stop,
            PlayerControlCommand::Shutdown => Self::Shutdown,
            PlayerControlCommand::RefreshDevices => Self::RefreshDevices,
            PlayerControlCommand::SeekMs => Self::SeekMs,
            PlayerControlCommand::SetVolume => Self::SetVolume,
            PlayerControlCommand::SetLfeMode => Self::SetLfeMode,
            PlayerControlCommand::SetOutputDevice => Self::SetOutputDevice,
            PlayerControlCommand::SetOutputOptions => Self::SetOutputOptions,
            PlayerControlCommand::SetOutputSinkRoute => Self::SetOutputSinkRoute,
            PlayerControlCommand::ClearOutputSinkRoute => Self::ClearOutputSinkRoute,
            PlayerControlCommand::PreloadTrack => Self::PreloadTrack,
            PlayerControlCommand::PreloadTrackRef => Self::PreloadTrackRef,
        }
    }
}

impl From<LibraryControlCommand> for ControlCommand {
    fn from(value: LibraryControlCommand) -> Self {
        match value {
            LibraryControlCommand::AddRoot => Self::AddRoot,
            LibraryControlCommand::RemoveRoot => Self::RemoveRoot,
            LibraryControlCommand::DeleteFolder => Self::DeleteFolder,
            LibraryControlCommand::RestoreFolder => Self::RestoreFolder,
            LibraryControlCommand::ScanAll => Self::ScanAll,
            LibraryControlCommand::ScanAllForce => Self::ScanAllForce,
            LibraryControlCommand::ListRoots => Self::ListRoots,
            LibraryControlCommand::ListFolders => Self::ListFolders,
            LibraryControlCommand::ListExcludedFolders => Self::ListExcludedFolders,
            LibraryControlCommand::ListTracks => Self::ListTracks,
            LibraryControlCommand::Search => Self::Search,
            LibraryControlCommand::ListPlaylists => Self::ListPlaylists,
            LibraryControlCommand::CreatePlaylist => Self::CreatePlaylist,
            LibraryControlCommand::RenamePlaylist => Self::RenamePlaylist,
            LibraryControlCommand::DeletePlaylist => Self::DeletePlaylist,
            LibraryControlCommand::ListPlaylistTracks => Self::ListPlaylistTracks,
            LibraryControlCommand::AddTrackToPlaylist => Self::AddTrackToPlaylist,
            LibraryControlCommand::AddTracksToPlaylist => Self::AddTracksToPlaylist,
            LibraryControlCommand::RemoveTrackFromPlaylist => Self::RemoveTrackFromPlaylist,
            LibraryControlCommand::RemoveTracksFromPlaylist => Self::RemoveTracksFromPlaylist,
            LibraryControlCommand::MoveTrackInPlaylist => Self::MoveTrackInPlaylist,
            LibraryControlCommand::ListLikedTrackIds => Self::ListLikedTrackIds,
            LibraryControlCommand::SetTrackLiked => Self::SetTrackLiked,
            LibraryControlCommand::Shutdown => Self::Shutdown,
        }
    }
}

impl PlayerControlCommand {
    #[flutter_rust_bridge::frb(ignore)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SwitchTrackRef => "switch_track_ref",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Stop => "stop",
            Self::Shutdown => "shutdown",
            Self::RefreshDevices => "refresh_devices",
            Self::SeekMs => "seek_ms",
            Self::SetVolume => "set_volume",
            Self::SetLfeMode => "set_lfe_mode",
            Self::SetOutputDevice => "set_output_device",
            Self::SetOutputOptions => "set_output_options",
            Self::SetOutputSinkRoute => "set_output_sink_route",
            Self::ClearOutputSinkRoute => "clear_output_sink_route",
            Self::PreloadTrack => "preload_track",
            Self::PreloadTrackRef => "preload_track_ref",
        }
    }

    #[flutter_rust_bridge::frb(ignore)]
    pub fn from_str(v: &str) -> Option<Self> {
        match v {
            "switch_track_ref" => Some(Self::SwitchTrackRef),
            "play" => Some(Self::Play),
            "pause" => Some(Self::Pause),
            "stop" => Some(Self::Stop),
            "shutdown" => Some(Self::Shutdown),
            "refresh_devices" => Some(Self::RefreshDevices),
            "seek_ms" => Some(Self::SeekMs),
            "set_volume" => Some(Self::SetVolume),
            "set_lfe_mode" => Some(Self::SetLfeMode),
            "set_output_device" => Some(Self::SetOutputDevice),
            "set_output_options" => Some(Self::SetOutputOptions),
            "set_output_sink_route" => Some(Self::SetOutputSinkRoute),
            "clear_output_sink_route" => Some(Self::ClearOutputSinkRoute),
            "preload_track" => Some(Self::PreloadTrack),
            "preload_track_ref" => Some(Self::PreloadTrackRef),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryControlCommand {
    AddRoot,
    RemoveRoot,
    DeleteFolder,
    RestoreFolder,
    ScanAll,
    ScanAllForce,
    ListRoots,
    ListFolders,
    ListExcludedFolders,
    ListTracks,
    Search,
    ListPlaylists,
    CreatePlaylist,
    RenamePlaylist,
    DeletePlaylist,
    ListPlaylistTracks,
    AddTrackToPlaylist,
    AddTracksToPlaylist,
    RemoveTrackFromPlaylist,
    RemoveTracksFromPlaylist,
    MoveTrackInPlaylist,
    ListLikedTrackIds,
    SetTrackLiked,
    Shutdown,
}

impl LibraryControlCommand {
    #[flutter_rust_bridge::frb(ignore)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AddRoot => "add_root",
            Self::RemoveRoot => "remove_root",
            Self::DeleteFolder => "delete_folder",
            Self::RestoreFolder => "restore_folder",
            Self::ScanAll => "scan_all",
            Self::ScanAllForce => "scan_all_force",
            Self::ListRoots => "list_roots",
            Self::ListFolders => "list_folders",
            Self::ListExcludedFolders => "list_excluded_folders",
            Self::ListTracks => "list_tracks",
            Self::Search => "search",
            Self::ListPlaylists => "list_playlists",
            Self::CreatePlaylist => "create_playlist",
            Self::RenamePlaylist => "rename_playlist",
            Self::DeletePlaylist => "delete_playlist",
            Self::ListPlaylistTracks => "list_playlist_tracks",
            Self::AddTrackToPlaylist => "add_track_to_playlist",
            Self::AddTracksToPlaylist => "add_tracks_to_playlist",
            Self::RemoveTrackFromPlaylist => "remove_track_from_playlist",
            Self::RemoveTracksFromPlaylist => "remove_tracks_from_playlist",
            Self::MoveTrackInPlaylist => "move_track_in_playlist",
            Self::ListLikedTrackIds => "list_liked_track_ids",
            Self::SetTrackLiked => "set_track_liked",
            Self::Shutdown => "shutdown",
        }
    }

    #[flutter_rust_bridge::frb(ignore)]
    pub fn from_str(v: &str) -> Option<Self> {
        match v {
            "add_root" => Some(Self::AddRoot),
            "remove_root" => Some(Self::RemoveRoot),
            "delete_folder" => Some(Self::DeleteFolder),
            "restore_folder" => Some(Self::RestoreFolder),
            "scan_all" => Some(Self::ScanAll),
            "scan_all_force" => Some(Self::ScanAllForce),
            "list_roots" => Some(Self::ListRoots),
            "list_folders" => Some(Self::ListFolders),
            "list_excluded_folders" => Some(Self::ListExcludedFolders),
            "list_tracks" => Some(Self::ListTracks),
            "search" => Some(Self::Search),
            "list_playlists" => Some(Self::ListPlaylists),
            "create_playlist" => Some(Self::CreatePlaylist),
            "rename_playlist" => Some(Self::RenamePlaylist),
            "delete_playlist" => Some(Self::DeletePlaylist),
            "list_playlist_tracks" => Some(Self::ListPlaylistTracks),
            "add_track_to_playlist" => Some(Self::AddTrackToPlaylist),
            "add_tracks_to_playlist" => Some(Self::AddTracksToPlaylist),
            "remove_track_from_playlist" => Some(Self::RemoveTrackFromPlaylist),
            "remove_tracks_from_playlist" => Some(Self::RemoveTracksFromPlaylist),
            "move_track_in_playlist" => Some(Self::MoveTrackInPlaylist),
            "list_liked_track_ids" => Some(Self::ListLikedTrackIds),
            "set_track_liked" => Some(Self::SetTrackLiked),
            "shutdown" => Some(Self::Shutdown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostEventTopic {
    #[serde(rename = "player.tick")]
    PlayerTick,
    #[serde(rename = "player.event")]
    PlayerEvent,
    #[serde(rename = "library.event")]
    LibraryEvent,
    #[serde(rename = "host.control.result")]
    HostControlResult,
    #[serde(rename = "host.control.finished")]
    HostControlFinished,
}

impl HostEventTopic {
    #[flutter_rust_bridge::frb(ignore)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PlayerTick => "player.tick",
            Self::PlayerEvent => "player.event",
            Self::LibraryEvent => "library.event",
            Self::HostControlResult => "host.control.result",
            Self::HostControlFinished => "host.control.finished",
        }
    }

    #[flutter_rust_bridge::frb(ignore)]
    pub fn from_str(v: &str) -> Option<Self> {
        match v {
            "player.tick" => Some(Self::PlayerTick),
            "player.event" => Some(Self::PlayerEvent),
            "library.event" => Some(Self::LibraryEvent),
            "host.control.result" => Some(Self::HostControlResult),
            "host.control.finished" => Some(Self::HostControlFinished),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostControlResultPayload {
    pub topic: HostEventTopic,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<RequestId>,
    pub scope: ControlScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<ControlCommand>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostControlFinishedPayload {
    pub topic: HostEventTopic,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<RequestId>,
    pub scope: ControlScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<ControlCommand>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostPlayerTickPayload {
    pub topic: HostEventTopic,
    pub state: PlayerState,
    pub position_ms: i64,
    pub track: Option<String>,
    pub wants_playback: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostPlayerEventEnvelope {
    pub topic: HostEventTopic,
    pub event: Event,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostLibraryEventEnvelope {
    pub topic: HostEventTopic,
    pub event: LibraryEvent,
}
