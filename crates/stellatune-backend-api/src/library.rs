use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use tokio::sync::broadcast;

use crate::runtime::{init_tracing, register_plugin_runtime_library};

use stellatune_core::{LibraryCommand, LibraryEvent};
use stellatune_library::{LibraryHandle, start_library};

pub struct LibraryService {
    instance_id: u64,
    handle: LibraryHandle,
}

impl LibraryService {
    pub async fn new(db_path: String) -> Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let instance_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        init_tracing();
        tracing::info!(instance_id, "creating library: {}", db_path);
        let handle = start_library(db_path).await?;
        register_plugin_runtime_library(handle.clone());
        Ok(Self {
            instance_id,
            handle,
        })
    }

    pub fn handle(&self) -> &LibraryHandle {
        &self.handle
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LibraryEvent> {
        self.handle.subscribe_events()
    }

    async fn dispatch(&self, cmd: LibraryCommand) -> Result<()> {
        self.handle
            .send_command(cmd)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn add_root(&self, path: String) -> Result<()> {
        self.dispatch(LibraryCommand::AddRoot { path }).await
    }

    pub async fn remove_root(&self, path: String) -> Result<()> {
        self.dispatch(LibraryCommand::RemoveRoot { path }).await
    }

    pub async fn delete_folder(&self, path: String) -> Result<()> {
        self.dispatch(LibraryCommand::DeleteFolder { path }).await
    }

    pub async fn restore_folder(&self, path: String) -> Result<()> {
        self.dispatch(LibraryCommand::RestoreFolder { path }).await
    }

    pub async fn list_excluded_folders(&self) -> Result<()> {
        self.dispatch(LibraryCommand::ListExcludedFolders).await
    }

    pub async fn scan_all(&self) -> Result<()> {
        self.dispatch(LibraryCommand::ScanAll).await
    }

    pub async fn scan_all_force(&self) -> Result<()> {
        self.dispatch(LibraryCommand::ScanAllForce).await
    }

    pub async fn list_roots(&self) -> Result<()> {
        self.dispatch(LibraryCommand::ListRoots).await
    }

    pub async fn list_folders(&self) -> Result<()> {
        self.dispatch(LibraryCommand::ListFolders).await
    }

    pub async fn list_tracks(
        &self,
        folder: String,
        recursive: bool,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<()> {
        self.dispatch(LibraryCommand::ListTracks {
            folder,
            recursive,
            query,
            limit,
            offset,
        })
        .await
    }

    pub async fn search(&self, query: String, limit: i64, offset: i64) -> Result<()> {
        self.dispatch(LibraryCommand::Search {
            query,
            limit,
            offset,
        })
        .await
    }

    pub async fn list_playlists(&self) -> Result<()> {
        self.dispatch(LibraryCommand::ListPlaylists).await
    }

    pub async fn create_playlist(&self, name: String) -> Result<()> {
        self.dispatch(LibraryCommand::CreatePlaylist { name }).await
    }

    pub async fn rename_playlist(&self, id: i64, name: String) -> Result<()> {
        self.dispatch(LibraryCommand::RenamePlaylist { id, name })
            .await
    }

    pub async fn delete_playlist(&self, id: i64) -> Result<()> {
        self.dispatch(LibraryCommand::DeletePlaylist { id }).await
    }

    pub async fn list_playlist_tracks(
        &self,
        playlist_id: i64,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<()> {
        self.dispatch(LibraryCommand::ListPlaylistTracks {
            playlist_id,
            query,
            limit,
            offset,
        })
        .await
    }

    pub async fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.dispatch(LibraryCommand::AddTrackToPlaylist {
            playlist_id,
            track_id,
        })
        .await
    }

    pub async fn add_tracks_to_playlist(
        &self,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) -> Result<()> {
        self.dispatch(LibraryCommand::AddTracksToPlaylist {
            playlist_id,
            track_ids,
        })
        .await
    }

    pub async fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.dispatch(LibraryCommand::RemoveTrackFromPlaylist {
            playlist_id,
            track_id,
        })
        .await
    }

    pub async fn remove_tracks_from_playlist(
        &self,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) -> Result<()> {
        self.dispatch(LibraryCommand::RemoveTracksFromPlaylist {
            playlist_id,
            track_ids,
        })
        .await
    }

    pub async fn move_track_in_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
        new_index: i64,
    ) -> Result<()> {
        self.dispatch(LibraryCommand::MoveTrackInPlaylist {
            playlist_id,
            track_id,
            new_index,
        })
        .await
    }

    pub async fn list_liked_track_ids(&self) -> Result<()> {
        self.dispatch(LibraryCommand::ListLikedTrackIds).await
    }

    pub async fn set_track_liked(&self, track_id: i64, liked: bool) -> Result<()> {
        self.dispatch(LibraryCommand::SetTrackLiked { track_id, liked })
            .await
    }

    pub async fn plugin_disable(&self, plugin_id: String) -> Result<()> {
        let report = crate::runtime::plugin_runtime_disable(&self.handle, plugin_id, 3_000).await?;
        if report.timed_out {
            return Err(anyhow::anyhow!(
                "plugin disable timed out: remaining_retired_leases={}",
                report.remaining_retired_leases
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

    pub async fn plugin_apply_state(&self) -> Result<()> {
        let report = crate::runtime::plugin_runtime_apply_state(&self.handle).await?;
        tracing::debug!(
            phase = report.phase,
            loaded = report.loaded,
            deactivated = report.deactivated,
            reclaimed_leases = report.reclaimed_leases,
            plan_actions_total = report.plan_actions_total,
            plan_load_new = report.plan_load_new,
            plan_reload_changed = report.plan_reload_changed,
            plan_deactivate = report.plan_deactivate,
            plan_ms = report.plan_ms,
            execute_ms = report.execute_ms,
            total_ms = report.total_ms,
            coalesced_requests = report.coalesced_requests,
            execution_loops = report.execution_loops,
            errors = report.errors.len(),
            "plugin_apply_state_done"
        );
        for err in report.errors {
            tracing::warn!(error = %err, "plugin_apply_state_error");
        }
        Ok(())
    }

    pub fn plugin_apply_state_status_json(&self) -> String {
        crate::runtime::plugin_runtime_apply_state_status_json()
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
