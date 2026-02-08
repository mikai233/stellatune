use anyhow::Result;
use crossbeam_channel::Receiver;

use crate::runtime::{init_tracing, register_plugin_runtime_library, shared_plugins};

use stellatune_core::{LibraryCommand, LibraryEvent};
use stellatune_library::{LibraryHandle, start_library_with_plugins};

pub struct LibraryService {
    handle: LibraryHandle,
}

impl LibraryService {
    pub fn new(db_path: String, disabled_plugin_ids: Vec<String>) -> Result<Self> {
        init_tracing();
        tracing::info!("creating library: {}", db_path);
        let handle = start_library_with_plugins(db_path, disabled_plugin_ids, shared_plugins())?;
        register_plugin_runtime_library(handle.clone());
        Ok(Self { handle })
    }

    pub fn handle(&self) -> &LibraryHandle {
        &self.handle
    }

    pub fn subscribe_events(&self) -> Receiver<LibraryEvent> {
        self.handle.subscribe_events()
    }

    pub fn add_root(&self, path: String) {
        self.handle.send_command(LibraryCommand::AddRoot { path });
    }

    pub fn remove_root(&self, path: String) {
        self.handle
            .send_command(LibraryCommand::RemoveRoot { path });
    }

    pub fn delete_folder(&self, path: String) {
        self.handle
            .send_command(LibraryCommand::DeleteFolder { path });
    }

    pub fn restore_folder(&self, path: String) {
        self.handle
            .send_command(LibraryCommand::RestoreFolder { path });
    }

    pub fn list_excluded_folders(&self) {
        self.handle
            .send_command(LibraryCommand::ListExcludedFolders);
    }

    pub fn scan_all(&self) {
        self.handle.send_command(LibraryCommand::ScanAll);
    }

    pub fn scan_all_force(&self) {
        self.handle.send_command(LibraryCommand::ScanAllForce);
    }

    pub fn list_roots(&self) {
        self.handle.send_command(LibraryCommand::ListRoots);
    }

    pub fn list_folders(&self) {
        self.handle.send_command(LibraryCommand::ListFolders);
    }

    pub fn list_tracks(
        &self,
        folder: String,
        recursive: bool,
        query: String,
        limit: i64,
        offset: i64,
    ) {
        self.handle.send_command(LibraryCommand::ListTracks {
            folder,
            recursive,
            query,
            limit,
            offset,
        });
    }

    pub fn search(&self, query: String, limit: i64, offset: i64) {
        self.handle.send_command(LibraryCommand::Search {
            query,
            limit,
            offset,
        });
    }

    pub fn list_playlists(&self) {
        self.handle.send_command(LibraryCommand::ListPlaylists);
    }

    pub fn create_playlist(&self, name: String) {
        self.handle
            .send_command(LibraryCommand::CreatePlaylist { name });
    }

    pub fn rename_playlist(&self, id: i64, name: String) {
        self.handle
            .send_command(LibraryCommand::RenamePlaylist { id, name });
    }

    pub fn delete_playlist(&self, id: i64) {
        self.handle
            .send_command(LibraryCommand::DeletePlaylist { id });
    }

    pub fn list_playlist_tracks(&self, playlist_id: i64, query: String, limit: i64, offset: i64) {
        self.handle
            .send_command(LibraryCommand::ListPlaylistTracks {
                playlist_id,
                query,
                limit,
                offset,
            });
    }

    pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) {
        self.handle
            .send_command(LibraryCommand::AddTrackToPlaylist {
                playlist_id,
                track_id,
            });
    }

    pub fn add_tracks_to_playlist(&self, playlist_id: i64, track_ids: Vec<i64>) {
        self.handle
            .send_command(LibraryCommand::AddTracksToPlaylist {
                playlist_id,
                track_ids,
            });
    }

    pub fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) {
        self.handle
            .send_command(LibraryCommand::RemoveTrackFromPlaylist {
                playlist_id,
                track_id,
            });
    }

    pub fn remove_tracks_from_playlist(&self, playlist_id: i64, track_ids: Vec<i64>) {
        self.handle
            .send_command(LibraryCommand::RemoveTracksFromPlaylist {
                playlist_id,
                track_ids,
            });
    }

    pub fn move_track_in_playlist(&self, playlist_id: i64, track_id: i64, new_index: i64) {
        self.handle
            .send_command(LibraryCommand::MoveTrackInPlaylist {
                playlist_id,
                track_id,
                new_index,
            });
    }

    pub fn list_liked_track_ids(&self) {
        self.handle.send_command(LibraryCommand::ListLikedTrackIds);
    }

    pub fn set_track_liked(&self, track_id: i64, liked: bool) {
        self.handle
            .send_command(LibraryCommand::SetTrackLiked { track_id, liked });
    }

    pub fn plugins_reload_with_disabled(&self, dir: String, disabled_ids: Vec<String>) {
        self.handle.plugins_reload_with_disabled(dir, disabled_ids);
    }
}
