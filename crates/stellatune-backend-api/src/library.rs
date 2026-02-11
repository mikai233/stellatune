use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use crossbeam_channel::Receiver;

use crate::runtime::{init_tracing, register_plugin_runtime_library};

use stellatune_core::{LibraryCommand, LibraryEvent};
use stellatune_library::{LibraryHandle, start_library};

pub struct LibraryService {
    instance_id: u64,
    handle: LibraryHandle,
}

impl LibraryService {
    pub fn new(db_path: String) -> Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let instance_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        init_tracing();
        tracing::info!(instance_id, "creating library: {}", db_path);
        let handle = start_library(db_path)?;
        register_plugin_runtime_library(handle.clone());
        Ok(Self {
            instance_id,
            handle,
        })
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

    pub async fn plugin_disable(&self, plugin_id: String) -> Result<()> {
        let report = crate::runtime::plugin_runtime_disable(&self.handle, plugin_id, 3_000).await?;
        if report.timed_out {
            return Err(anyhow::anyhow!(
                "plugin disable timed out: remaining_draining_generations={}",
                report.remaining_draining_generations
            ));
        }
        if !report.errors.is_empty() {
            return Err(anyhow::anyhow!(
                "plugin disable finished with errors: {}",
                report.errors.join("; ")
            ));
        }
        Ok(())
    }

    pub async fn plugin_enable(&self, plugin_id: String) -> Result<()> {
        let report = crate::runtime::plugin_runtime_enable(&self.handle, plugin_id).await?;
        tracing::debug!(
            plugin_id = report.plugin_id,
            phase = report.phase,
            "plugin_enable_done"
        );
        Ok(())
    }

    pub async fn plugins_reload_from_state(&self, dir: String) -> Result<()> {
        let report = crate::runtime::plugin_runtime_reload_from_state(&self.handle, dir).await?;
        tracing::debug!(phase = report.phase, "plugin_reload_from_state_done");
        Ok(())
    }

    pub async fn list_disabled_plugin_ids(&self) -> Result<Vec<String>> {
        self.handle.list_disabled_plugin_ids().await
    }
}

impl Drop for LibraryService {
    fn drop(&mut self) {
        tracing::info!(instance_id = self.instance_id, "dropping library");
    }
}
