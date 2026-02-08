use std::thread;

use crate::frb_generated::{RustOpaque, StreamSink};
use anyhow::Result;

use stellatune_backend_api::library::LibraryService;
use stellatune_core::LibraryEvent;

pub struct Library {
    service: LibraryService,
}

impl Library {
    fn new(db_path: String, disabled_plugin_ids: Vec<String>) -> Result<Self> {
        let service = LibraryService::new(db_path, disabled_plugin_ids)?;
        Ok(Self { service })
    }
}

pub fn create_library(
    db_path: String,
    disabled_plugin_ids: Vec<String>,
) -> Result<RustOpaque<Library>> {
    Ok(RustOpaque::new(Library::new(db_path, disabled_plugin_ids)?))
}

pub fn library_add_root(library: RustOpaque<Library>, path: String) {
    library.service.add_root(path);
}

pub fn library_remove_root(library: RustOpaque<Library>, path: String) {
    library.service.remove_root(path);
}

pub fn library_delete_folder(library: RustOpaque<Library>, path: String) {
    library.service.delete_folder(path);
}

pub fn library_restore_folder(library: RustOpaque<Library>, path: String) {
    library.service.restore_folder(path);
}

pub fn library_list_excluded_folders(library: RustOpaque<Library>) {
    library.service.list_excluded_folders();
}

pub fn library_scan_all(library: RustOpaque<Library>) {
    library.service.scan_all();
}

pub fn library_scan_all_force(library: RustOpaque<Library>) {
    library.service.scan_all_force();
}

pub fn library_list_roots(library: RustOpaque<Library>) {
    library.service.list_roots();
}

pub fn library_list_folders(library: RustOpaque<Library>) {
    library.service.list_folders();
}

pub fn library_list_tracks(
    library: RustOpaque<Library>,
    folder: String,
    recursive: bool,
    query: String,
    limit: i64,
    offset: i64,
) {
    library
        .service
        .list_tracks(folder, recursive, query, limit, offset);
}

pub fn library_search(library: RustOpaque<Library>, query: String, limit: i64, offset: i64) {
    library.service.search(query, limit, offset);
}

pub fn library_list_playlists(library: RustOpaque<Library>) {
    library.service.list_playlists();
}

pub fn library_create_playlist(library: RustOpaque<Library>, name: String) {
    library.service.create_playlist(name);
}

pub fn library_rename_playlist(library: RustOpaque<Library>, id: i64, name: String) {
    library.service.rename_playlist(id, name);
}

pub fn library_delete_playlist(library: RustOpaque<Library>, id: i64) {
    library.service.delete_playlist(id);
}

pub fn library_list_playlist_tracks(
    library: RustOpaque<Library>,
    playlist_id: i64,
    query: String,
    limit: i64,
    offset: i64,
) {
    library
        .service
        .list_playlist_tracks(playlist_id, query, limit, offset);
}

pub fn library_add_track_to_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
) {
    library.service.add_track_to_playlist(playlist_id, track_id);
}

pub fn library_add_tracks_to_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) {
    library
        .service
        .add_tracks_to_playlist(playlist_id, track_ids);
}

pub fn library_remove_track_from_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
) {
    library
        .service
        .remove_track_from_playlist(playlist_id, track_id);
}

pub fn library_remove_tracks_from_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) {
    library
        .service
        .remove_tracks_from_playlist(playlist_id, track_ids);
}

pub fn library_move_track_in_playlist(
    library: RustOpaque<Library>,
    playlist_id: i64,
    track_id: i64,
    new_index: i64,
) {
    library
        .service
        .move_track_in_playlist(playlist_id, track_id, new_index);
}

pub fn library_list_liked_track_ids(library: RustOpaque<Library>) {
    library.service.list_liked_track_ids();
}

pub fn library_set_track_liked(library: RustOpaque<Library>, track_id: i64, liked: bool) {
    library.service.set_track_liked(track_id, liked);
}

pub fn library_events(library: RustOpaque<Library>, sink: StreamSink<LibraryEvent>) -> Result<()> {
    let rx = library.service.subscribe_events();

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
        .service
        .plugins_reload_with_disabled(dir, disabled_ids);
}
