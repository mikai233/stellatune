use stellatune_core::{
    Command, ControlCommand, ControlScope, Event, LibraryCommand, LibraryControlCommand,
    LibraryEvent, PlayerControlCommand,
};

use super::types::ControlWaitKind;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn control_wait_kind(root: &serde_json::Value) -> ControlWaitKind {
    let scope = parse_control_scope(root).unwrap_or(ControlScope::Player);
    let command = root.get("command").and_then(|v| v.as_str());

    match scope {
        ControlScope::Player => match command.and_then(PlayerControlCommand::from_str) {
            Some(PlayerControlCommand::Play) => {
                ControlWaitKind::PlayerState(stellatune_core::PlayerState::Playing)
            }
            Some(PlayerControlCommand::Pause) => {
                ControlWaitKind::PlayerState(stellatune_core::PlayerState::Paused)
            }
            Some(PlayerControlCommand::Stop) => {
                ControlWaitKind::PlayerState(stellatune_core::PlayerState::Stopped)
            }
            Some(PlayerControlCommand::SeekMs) => ControlWaitKind::PlayerPosition,
            Some(PlayerControlCommand::SetVolume) => ControlWaitKind::PlayerVolume,
            Some(PlayerControlCommand::LoadTrack | PlayerControlCommand::LoadTrackRef) => {
                ControlWaitKind::PlayerTrackChanged
            }
            Some(PlayerControlCommand::RefreshDevices) => ControlWaitKind::PlayerDevicesChanged,
            _ => ControlWaitKind::Immediate,
        },
        ControlScope::Library => match command.and_then(LibraryControlCommand::from_str) {
            Some(LibraryControlCommand::ListRoots) => ControlWaitKind::LibraryRoots,
            Some(LibraryControlCommand::ListFolders) => ControlWaitKind::LibraryFolders,
            Some(LibraryControlCommand::ListExcludedFolders) => {
                ControlWaitKind::LibraryExcludedFolders
            }
            Some(LibraryControlCommand::ListTracks) => ControlWaitKind::LibraryTracks,
            Some(LibraryControlCommand::Search) => ControlWaitKind::LibrarySearchResult,
            Some(LibraryControlCommand::ListPlaylists) => ControlWaitKind::LibraryPlaylists,
            Some(LibraryControlCommand::ListPlaylistTracks) => {
                ControlWaitKind::LibraryPlaylistTracks
            }
            Some(LibraryControlCommand::ListLikedTrackIds) => ControlWaitKind::LibraryLikedTrackIds,
            Some(LibraryControlCommand::ScanAll | LibraryControlCommand::ScanAllForce) => {
                ControlWaitKind::LibraryScanFinished
            }
            Some(
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
                | LibraryControlCommand::SetTrackLiked,
            ) => ControlWaitKind::LibraryChanged,
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
        (ControlWaitKind::PlayerDevicesChanged, Event::OutputDevicesChanged { .. }) => true,
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
pub(super) fn parse_player_control(root: &serde_json::Value) -> Result<Option<Command>, String> {
    let Some(raw_command) = root.get("command").and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let Some(command) = PlayerControlCommand::from_str(raw_command) else {
        return Err(format!("unsupported player command `{raw_command}`"));
    };
    match command {
        PlayerControlCommand::LoadTrack => Ok(Some(Command::LoadTrack {
            path: json_req_string(root, "path")?,
        })),
        PlayerControlCommand::LoadTrackRef => Ok(Some(Command::LoadTrackRef {
            track: json_req_track_ref(root, "track")?,
        })),
        PlayerControlCommand::Play => Ok(Some(Command::Play)),
        PlayerControlCommand::Pause => Ok(Some(Command::Pause)),
        PlayerControlCommand::Stop => Ok(Some(Command::Stop)),
        PlayerControlCommand::Shutdown => Ok(Some(Command::Shutdown)),
        PlayerControlCommand::RefreshDevices => Ok(Some(Command::RefreshDevices)),
        PlayerControlCommand::SeekMs => {
            let position_ms = json_req_u64(root, "position_ms")?;
            Ok(Some(Command::SeekMs { position_ms }))
        }
        PlayerControlCommand::SetVolume => {
            let volume = root
                .get("volume")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| "missing `volume`".to_string())? as f32;
            Ok(Some(Command::SetVolume { volume }))
        }
        PlayerControlCommand::SetLfeMode => Ok(Some(Command::SetLfeMode {
            mode: parse_lfe_mode(root, "mode")?,
        })),
        PlayerControlCommand::SetOutputDevice => Ok(Some(Command::SetOutputDevice {
            backend: parse_audio_backend(root, "backend")?,
            device_id: root
                .get("device_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        })),
        PlayerControlCommand::SetOutputOptions => Ok(Some(Command::SetOutputOptions {
            match_track_sample_rate: json_req_bool(root, "match_track_sample_rate")?,
            gapless_playback: json_req_bool(root, "gapless_playback")?,
            seek_track_fade: json_req_bool(root, "seek_track_fade")?,
        })),
        PlayerControlCommand::SetOutputSinkRoute => Ok(Some(Command::SetOutputSinkRoute {
            route: json_req_output_sink_route(root, "route")?,
        })),
        PlayerControlCommand::ClearOutputSinkRoute => Ok(Some(Command::ClearOutputSinkRoute)),
        PlayerControlCommand::PreloadTrack => Ok(Some(Command::PreloadTrack {
            path: json_req_string(root, "path")?,
            position_ms: root
                .get("position_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        })),
        PlayerControlCommand::PreloadTrackRef => Ok(Some(Command::PreloadTrackRef {
            track: json_req_track_ref(root, "track")?,
            position_ms: root
                .get("position_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        })),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_req_i64(root: &serde_json::Value, key: &str) -> Result<i64, String> {
    root.get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| format!("missing `{key}`"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_req_u64(root: &serde_json::Value, key: &str) -> Result<u64, String> {
    root.get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("missing `{key}`"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_req_bool(root: &serde_json::Value, key: &str) -> Result<bool, String> {
    root.get(key)
        .and_then(|v| v.as_bool())
        .ok_or_else(|| format!("missing `{key}`"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_req_string(root: &serde_json::Value, key: &str) -> Result<String, String> {
    root.get(key)
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .ok_or_else(|| format!("missing `{key}`"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_req_track_ref(
    root: &serde_json::Value,
    key: &str,
) -> Result<stellatune_core::TrackRef, String> {
    let value = root.get(key).ok_or_else(|| format!("missing `{key}`"))?;
    serde_json::from_value::<stellatune_core::TrackRef>(value.clone())
        .map_err(|e| format!("invalid `{key}`: {e}"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_req_output_sink_route(
    root: &serde_json::Value,
    key: &str,
) -> Result<stellatune_core::OutputSinkRoute, String> {
    let value = root.get(key).ok_or_else(|| format!("missing `{key}`"))?;
    serde_json::from_value::<stellatune_core::OutputSinkRoute>(value.clone())
        .map_err(|e| format!("invalid `{key}`: {e}"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_opt_bool(root: &serde_json::Value, key: &str, default: bool) -> bool {
    root.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_opt_i64(root: &serde_json::Value, key: &str, default: i64) -> i64 {
    root.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_opt_string(root: &serde_json::Value, key: &str, default: &str) -> String {
    root.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn json_req_i64_vec(root: &serde_json::Value, key: &str) -> Result<Vec<i64>, String> {
    let arr = root
        .get(key)
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("missing `{key}`"))?;
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        let Some(n) = v.as_i64() else {
            return Err(format!("`{key}` contains non-integer item"));
        };
        out.push(n);
    }
    Ok(out)
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn parse_audio_backend(
    root: &serde_json::Value,
    key: &str,
) -> Result<stellatune_core::AudioBackend, String> {
    if let Some(s) = root.get(key).and_then(|v| v.as_str()) {
        return match s {
            "shared" | "Shared" => Ok(stellatune_core::AudioBackend::Shared),
            "wasapi_exclusive" | "WasapiExclusive" => {
                Ok(stellatune_core::AudioBackend::WasapiExclusive)
            }
            "asio" | "Asio" => Ok(stellatune_core::AudioBackend::Asio),
            _ => Err(format!("invalid `{key}`")),
        };
    }
    let value = root.get(key).ok_or_else(|| format!("missing `{key}`"))?;
    serde_json::from_value::<stellatune_core::AudioBackend>(value.clone())
        .map_err(|e| format!("invalid `{key}`: {e}"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn parse_lfe_mode(root: &serde_json::Value, key: &str) -> Result<stellatune_core::LfeMode, String> {
    if let Some(s) = root.get(key).and_then(|v| v.as_str()) {
        return match s {
            "mute" | "Mute" => Ok(stellatune_core::LfeMode::Mute),
            "mix_to_front" | "MixToFront" => Ok(stellatune_core::LfeMode::MixToFront),
            _ => Err(format!("invalid `{key}`")),
        };
    }
    let value = root.get(key).ok_or_else(|| format!("missing `{key}`"))?;
    serde_json::from_value::<stellatune_core::LfeMode>(value.clone())
        .map_err(|e| format!("invalid `{key}`: {e}"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn parse_control_scope(root: &serde_json::Value) -> Result<ControlScope, String> {
    let scope = root
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or(ControlScope::Player.as_str());
    ControlScope::from_str(scope).ok_or_else(|| format!("unsupported scope `{scope}`"))
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn control_scope_from_root(root: Option<&serde_json::Value>) -> ControlScope {
    root.and_then(|v| v.get("scope"))
        .and_then(|v| v.as_str())
        .and_then(ControlScope::from_str)
        .unwrap_or(ControlScope::Player)
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn control_command_from_root(
    root: Option<&serde_json::Value>,
    scope: ControlScope,
) -> Option<ControlCommand> {
    let raw = root
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str())?;
    match scope {
        ControlScope::Player => PlayerControlCommand::from_str(raw).map(Into::into),
        ControlScope::Library => LibraryControlCommand::from_str(raw).map(Into::into),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn parse_library_control(root: &serde_json::Value) -> Result<Option<LibraryCommand>, String> {
    let Some(raw_command) = root.get("command").and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let Some(command) = LibraryControlCommand::from_str(raw_command) else {
        return Err(format!("unsupported library command `{raw_command}`"));
    };
    match command {
        LibraryControlCommand::AddRoot => Ok(Some(LibraryCommand::AddRoot {
            path: json_req_string(root, "path")?,
        })),
        LibraryControlCommand::RemoveRoot => Ok(Some(LibraryCommand::RemoveRoot {
            path: json_req_string(root, "path")?,
        })),
        LibraryControlCommand::DeleteFolder => Ok(Some(LibraryCommand::DeleteFolder {
            path: json_req_string(root, "path")?,
        })),
        LibraryControlCommand::RestoreFolder => Ok(Some(LibraryCommand::RestoreFolder {
            path: json_req_string(root, "path")?,
        })),
        LibraryControlCommand::ScanAll => Ok(Some(LibraryCommand::ScanAll)),
        LibraryControlCommand::ScanAllForce => Ok(Some(LibraryCommand::ScanAllForce)),
        LibraryControlCommand::ListRoots => Ok(Some(LibraryCommand::ListRoots)),
        LibraryControlCommand::ListFolders => Ok(Some(LibraryCommand::ListFolders)),
        LibraryControlCommand::ListExcludedFolders => Ok(Some(LibraryCommand::ListExcludedFolders)),
        LibraryControlCommand::ListTracks => Ok(Some(LibraryCommand::ListTracks {
            folder: json_opt_string(root, "folder", ""),
            recursive: json_opt_bool(root, "recursive", true),
            query: json_opt_string(root, "query", ""),
            limit: json_opt_i64(root, "limit", 5000),
            offset: json_opt_i64(root, "offset", 0),
        })),
        LibraryControlCommand::Search => Ok(Some(LibraryCommand::Search {
            query: json_opt_string(root, "query", ""),
            limit: json_opt_i64(root, "limit", 200),
            offset: json_opt_i64(root, "offset", 0),
        })),
        LibraryControlCommand::ListPlaylists => Ok(Some(LibraryCommand::ListPlaylists)),
        LibraryControlCommand::CreatePlaylist => Ok(Some(LibraryCommand::CreatePlaylist {
            name: json_req_string(root, "name")?,
        })),
        LibraryControlCommand::RenamePlaylist => Ok(Some(LibraryCommand::RenamePlaylist {
            id: json_req_i64(root, "id")?,
            name: json_req_string(root, "name")?,
        })),
        LibraryControlCommand::DeletePlaylist => Ok(Some(LibraryCommand::DeletePlaylist {
            id: json_req_i64(root, "id")?,
        })),
        LibraryControlCommand::ListPlaylistTracks => Ok(Some(LibraryCommand::ListPlaylistTracks {
            playlist_id: json_req_i64(root, "playlist_id")?,
            query: json_opt_string(root, "query", ""),
            limit: json_opt_i64(root, "limit", 5000),
            offset: json_opt_i64(root, "offset", 0),
        })),
        LibraryControlCommand::AddTrackToPlaylist => Ok(Some(LibraryCommand::AddTrackToPlaylist {
            playlist_id: json_req_i64(root, "playlist_id")?,
            track_id: json_req_i64(root, "track_id")?,
        })),
        LibraryControlCommand::AddTracksToPlaylist => {
            Ok(Some(LibraryCommand::AddTracksToPlaylist {
                playlist_id: json_req_i64(root, "playlist_id")?,
                track_ids: json_req_i64_vec(root, "track_ids")?,
            }))
        }
        LibraryControlCommand::RemoveTrackFromPlaylist => {
            Ok(Some(LibraryCommand::RemoveTrackFromPlaylist {
                playlist_id: json_req_i64(root, "playlist_id")?,
                track_id: json_req_i64(root, "track_id")?,
            }))
        }
        LibraryControlCommand::RemoveTracksFromPlaylist => {
            Ok(Some(LibraryCommand::RemoveTracksFromPlaylist {
                playlist_id: json_req_i64(root, "playlist_id")?,
                track_ids: json_req_i64_vec(root, "track_ids")?,
            }))
        }
        LibraryControlCommand::MoveTrackInPlaylist => {
            Ok(Some(LibraryCommand::MoveTrackInPlaylist {
                playlist_id: json_req_i64(root, "playlist_id")?,
                track_id: json_req_i64(root, "track_id")?,
                new_index: json_req_i64(root, "new_index")?,
            }))
        }
        LibraryControlCommand::ListLikedTrackIds => Ok(Some(LibraryCommand::ListLikedTrackIds)),
        LibraryControlCommand::SetTrackLiked => Ok(Some(LibraryCommand::SetTrackLiked {
            track_id: json_req_i64(root, "track_id")?,
            liked: json_opt_bool(root, "liked", false),
        })),
        LibraryControlCommand::Shutdown => Ok(Some(LibraryCommand::Shutdown)),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn route_plugin_control_root(
    root: &serde_json::Value,
    engine: Option<&stellatune_audio::EngineHandle>,
    library: Option<&stellatune_library::LibraryHandle>,
) -> Result<(), String> {
    match parse_control_scope(root)? {
        ControlScope::Player => {
            let Some(engine) = engine else {
                return Err("player unavailable".to_string());
            };
            if let Some(cmd) = parse_player_control(root)? {
                engine.send_command(cmd);
            }
            Ok(())
        }
        ControlScope::Library => {
            let Some(library) = library else {
                return Err("library unavailable".to_string());
            };
            if let Some(cmd) = parse_library_control(root)? {
                library.send_command(cmd);
            }
            Ok(())
        }
    }
}
