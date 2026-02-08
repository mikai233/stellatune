use std::thread;

use crate::frb_generated::{RustOpaque, StreamSink};
use anyhow::Result;

use super::runtime::{init_tracing, register_plugin_runtime_library, shared_plugins};

use stellatune_core::{LibraryCommand, LibraryEvent};
use stellatune_library::start_library_with_plugins;

pub struct Library {
    handle: stellatune_library::LibraryHandle,
}

impl Library {
    fn new(db_path: String, disabled_plugin_ids: Vec<String>) -> Result<Self> {
        init_tracing();
        tracing::info!("creating library: {}", db_path);
        let handle = start_library_with_plugins(db_path, disabled_plugin_ids, shared_plugins())?;
        register_plugin_runtime_library(handle.clone());
        Ok(Self { handle })
    }
}

pub fn create_library(
    db_path: String,
    disabled_plugin_ids: Vec<String>,
) -> Result<RustOpaque<Library>> {
    Ok(RustOpaque::new(Library::new(db_path, disabled_plugin_ids)?))
}

pub fn library_add_root(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::AddRoot { path });
}

pub fn library_remove_root(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::RemoveRoot { path });
}

pub fn library_delete_folder(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::DeleteFolder { path });
}

pub fn library_restore_folder(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::RestoreFolder { path });
}

pub fn library_list_excluded_folders(library: RustOpaque<Library>) {
    library
        .handle
        .send_command(LibraryCommand::ListExcludedFolders);
}

pub fn library_scan_all(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ScanAll);
}

pub fn library_scan_all_force(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ScanAllForce);
}

pub fn library_list_roots(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ListRoots);
}

pub fn library_list_folders(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ListFolders);
}

pub fn library_list_tracks(
    library: RustOpaque<Library>,
    folder: String,
    recursive: bool,
    query: String,
    limit: i64,
    offset: i64,
) {
    library.handle.send_command(LibraryCommand::ListTracks {
        folder,
        recursive,
        query,
        limit,
        offset,
    });
}

pub fn library_search(library: RustOpaque<Library>, query: String, limit: i64, offset: i64) {
    library.handle.send_command(LibraryCommand::Search {
        query,
        limit,
        offset,
    });
}

pub fn library_list_playlists(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ListPlaylists);
}

pub fn library_create_playlist(library: RustOpaque<Library>, name: String) {
    library
        .handle
        .send_command(LibraryCommand::CreatePlaylist { name });
}

pub fn library_rename_playlist(library: RustOpaque<Library>, id: i64, name: String) {
    library
        .handle
        .send_command(LibraryCommand::RenamePlaylist { id, name });
}

pub fn library_delete_playlist(library: RustOpaque<Library>, id: i64) {
    library
        .handle
        .send_command(LibraryCommand::DeletePlaylist { id });
}

pub fn library_list_playlist_tracks(
    library: RustOpaque<Library>,
    playlist_id: i64,
    query: String,
    limit: i64,
    offset: i64,
) {
    library
        .handle
        .send_command(LibraryCommand::ListPlaylistTracks {
            playlist_id,
            query,
            limit,
            offset,
        });
}

pub fn library_add_track_to_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
) {
    library
        .handle
        .send_command(LibraryCommand::AddTrackToPlaylist {
            playlist_id,
            track_id,
        });
}

pub fn library_add_tracks_to_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) {
    library
        .handle
        .send_command(LibraryCommand::AddTracksToPlaylist {
            playlist_id,
            track_ids,
        });
}

pub fn library_remove_track_from_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
) {
    library
        .handle
        .send_command(LibraryCommand::RemoveTrackFromPlaylist {
            playlist_id,
            track_id,
        });
}

pub fn library_remove_tracks_from_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) {
    library
        .handle
        .send_command(LibraryCommand::RemoveTracksFromPlaylist {
            playlist_id,
            track_ids,
        });
}

pub fn library_move_track_in_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
    new_index: i64,
) {
    library
        .handle
        .send_command(LibraryCommand::MoveTrackInPlaylist {
            playlist_id,
            track_id,
            new_index,
        });
}

pub fn library_list_liked_track_ids(library: RustOpaque<Library>) {
    library
        .handle
        .send_command(LibraryCommand::ListLikedTrackIds);
}

pub fn library_set_track_liked(library: RustOpaque<Library>, track_id: i64, liked: bool) {
    library
        .handle
        .send_command(LibraryCommand::SetTrackLiked { track_id, liked });
}

pub fn library_events(library: RustOpaque<Library>, sink: StreamSink<LibraryEvent>) -> Result<()> {
    let rx = library.handle.subscribe_events();

    thread::Builder::new()
        .name("stellatune-library-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn stellatune-library-events thread");

    Ok(())
}

pub fn library_plugins_reload_with_disabled(
    library: RustOpaque<Library>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    library
        .handle
        .plugins_reload_with_disabled(dir, disabled_ids);
}
