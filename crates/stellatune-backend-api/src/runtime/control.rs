use stellatune_core::{
    ControlScope, Event, LibraryControlCommand, LibraryEvent, PlayerControlCommand,
};
use stellatune_plugin_protocol::PluginControlRequest;

use super::types::ControlWaitKind;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn control_wait_kind(request: &PluginControlRequest) -> ControlWaitKind {
    match request {
        PluginControlRequest::Player { control, .. } => match control.command() {
            PlayerControlCommand::Play => {
                ControlWaitKind::PlayerState(stellatune_core::PlayerState::Playing)
            }
            PlayerControlCommand::Pause => {
                ControlWaitKind::PlayerState(stellatune_core::PlayerState::Paused)
            }
            PlayerControlCommand::Stop => {
                ControlWaitKind::PlayerState(stellatune_core::PlayerState::Stopped)
            }
            PlayerControlCommand::SeekMs => ControlWaitKind::PlayerPosition,
            PlayerControlCommand::SetVolume => ControlWaitKind::PlayerVolume,
            PlayerControlCommand::SwitchTrackRef => ControlWaitKind::PlayerTrackChanged,
            PlayerControlCommand::RefreshDevices => ControlWaitKind::Immediate,
            _ => ControlWaitKind::Immediate,
        },
        PluginControlRequest::Library { control, .. } => match control.command() {
            LibraryControlCommand::ListRoots => ControlWaitKind::LibraryRoots,
            LibraryControlCommand::ListFolders => ControlWaitKind::LibraryFolders,
            LibraryControlCommand::ListExcludedFolders => ControlWaitKind::LibraryExcludedFolders,
            LibraryControlCommand::ListTracks => ControlWaitKind::LibraryTracks,
            LibraryControlCommand::Search => ControlWaitKind::LibrarySearchResult,
            LibraryControlCommand::ListPlaylists => ControlWaitKind::LibraryPlaylists,
            LibraryControlCommand::ListPlaylistTracks => ControlWaitKind::LibraryPlaylistTracks,
            LibraryControlCommand::ListLikedTrackIds => ControlWaitKind::LibraryLikedTrackIds,
            LibraryControlCommand::ScanAll | LibraryControlCommand::ScanAllForce => {
                ControlWaitKind::LibraryScanFinished
            }
            LibraryControlCommand::AddRoot
            | LibraryControlCommand::RemoveRoot
            | LibraryControlCommand::DeleteFolder
            | LibraryControlCommand::RestoreFolder
            | LibraryControlCommand::CreatePlaylist
            | LibraryControlCommand::RenamePlaylist
            | LibraryControlCommand::DeletePlaylist
            | LibraryControlCommand::AddTrackToPlaylist
            | LibraryControlCommand::AddTracksToPlaylist
            | LibraryControlCommand::RemoveTrackFromPlaylist
            | LibraryControlCommand::RemoveTracksFromPlaylist
            | LibraryControlCommand::MoveTrackInPlaylist
            | LibraryControlCommand::SetTrackLiked => ControlWaitKind::LibraryChanged,
            _ => ControlWaitKind::Immediate,
        },
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn is_wait_satisfied_by_player(wait: ControlWaitKind, event: &Event) -> bool {
    match (wait, event) {
        (ControlWaitKind::PlayerState(expected), Event::StateChanged { state }) => {
            *state == expected
        }
        (ControlWaitKind::PlayerPosition, Event::Position { .. }) => true,
        (ControlWaitKind::PlayerVolume, Event::VolumeChanged { .. }) => true,
        (ControlWaitKind::PlayerTrackChanged, Event::TrackChanged { .. }) => true,
        _ => false,
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn is_wait_satisfied_by_library(wait: ControlWaitKind, event: &LibraryEvent) -> bool {
    matches!(
        (wait, event),
        (ControlWaitKind::LibraryRoots, LibraryEvent::Roots { .. })
            | (
                ControlWaitKind::LibraryFolders,
                LibraryEvent::Folders { .. }
            )
            | (
                ControlWaitKind::LibraryExcludedFolders,
                LibraryEvent::ExcludedFolders { .. }
            )
            | (ControlWaitKind::LibraryTracks, LibraryEvent::Tracks { .. })
            | (
                ControlWaitKind::LibrarySearchResult,
                LibraryEvent::SearchResult { .. }
            )
            | (
                ControlWaitKind::LibraryPlaylists,
                LibraryEvent::Playlists { .. }
            )
            | (
                ControlWaitKind::LibraryPlaylistTracks,
                LibraryEvent::PlaylistTracks { .. }
            )
            | (
                ControlWaitKind::LibraryLikedTrackIds,
                LibraryEvent::LikedTrackIds { .. }
            )
            | (
                ControlWaitKind::LibraryScanFinished,
                LibraryEvent::ScanFinished { .. }
            )
            | (ControlWaitKind::LibraryChanged, LibraryEvent::Changed)
    )
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn route_plugin_control_request(
    request: &PluginControlRequest,
    engine: Option<&stellatune_audio::EngineHandle>,
    library: Option<&stellatune_library::LibraryHandle>,
) -> Result<(), String> {
    match request {
        PluginControlRequest::Player { control, .. } => {
            let Some(engine) = engine else {
                return Err("player unavailable".to_string());
            };
            engine.dispatch_command_blocking(control.to_command())
        }
        PluginControlRequest::Library { control, .. } => {
            let Some(library) = library else {
                return Err("library unavailable".to_string());
            };
            library.send_command(control.to_command());
            Ok(())
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn control_scope_from_request(request: Option<&PluginControlRequest>) -> ControlScope {
    request
        .map(PluginControlRequest::scope)
        .unwrap_or(ControlScope::Player)
}
