use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use tokio::sync::broadcast;

use crate::runtime::init_tracing;

use stellatune_library::{LibraryEvent, LibraryHandle, PlaylistLite, TrackLite, start_library};

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

    pub async fn add_root(&self, path: String) -> Result<()> {
        self.handle.add_root(path).await.map_err(anyhow::Error::msg)
    }

    pub async fn remove_root(&self, path: String) -> Result<()> {
        self.handle
            .remove_root(path)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn delete_folder(&self, path: String) -> Result<()> {
        self.handle
            .delete_folder(path)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn restore_folder(&self, path: String) -> Result<()> {
        self.handle
            .restore_folder(path)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn list_excluded_folders(&self) -> Result<Vec<String>> {
        self.handle.list_excluded_folders().await
    }

    pub async fn scan_all(&self) -> Result<()> {
        self.handle.scan_all().await.map_err(anyhow::Error::msg)
    }

    pub async fn scan_all_force(&self) -> Result<()> {
        self.handle
            .scan_all_force()
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn list_roots(&self) -> Result<Vec<String>> {
        self.handle.list_roots().await
    }

    pub async fn list_folders(&self) -> Result<Vec<String>> {
        self.handle.list_folders().await
    }

    pub async fn list_tracks(
        &self,
        folder: String,
        recursive: bool,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TrackLite>> {
        self.handle
            .list_tracks(folder, recursive, query, limit, offset)
            .await
    }

    pub async fn search(&self, query: String, limit: i64, offset: i64) -> Result<Vec<TrackLite>> {
        self.handle.search(query, limit, offset).await
    }

    pub async fn list_playlists(&self) -> Result<Vec<PlaylistLite>> {
        self.handle.list_playlists().await
    }

    pub async fn create_playlist(&self, name: String) -> Result<()> {
        self.handle
            .create_playlist(name)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn rename_playlist(&self, id: i64, name: String) -> Result<()> {
        self.handle
            .rename_playlist(id, name)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn delete_playlist(&self, id: i64) -> Result<()> {
        self.handle
            .delete_playlist(id)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn list_playlist_tracks(
        &self,
        playlist_id: i64,
        query: String,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TrackLite>> {
        self.handle
            .list_playlist_tracks(playlist_id, query, limit, offset)
            .await
    }

    pub async fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.handle
            .add_track_to_playlist(playlist_id, track_id)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn add_tracks_to_playlist(
        &self,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) -> Result<()> {
        self.handle
            .add_tracks_to_playlist(playlist_id, track_ids)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.handle
            .remove_track_from_playlist(playlist_id, track_id)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn remove_tracks_from_playlist(
        &self,
        playlist_id: i64,
        track_ids: Vec<i64>,
    ) -> Result<()> {
        self.handle
            .remove_tracks_from_playlist(playlist_id, track_ids)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn move_track_in_playlist(
        &self,
        playlist_id: i64,
        track_id: i64,
        new_index: i64,
    ) -> Result<()> {
        self.handle
            .move_track_in_playlist(playlist_id, track_id, new_index)
            .await
            .map_err(anyhow::Error::msg)
    }

    pub async fn list_liked_track_ids(&self) -> Result<Vec<i64>> {
        self.handle.list_liked_track_ids().await
    }

    pub async fn set_track_liked(&self, track_id: i64, liked: bool) -> Result<()> {
        self.handle
            .set_track_liked(track_id, liked)
            .await
            .map_err(anyhow::Error::msg)
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

    pub async fn plugin_apply_state_status_json(&self) -> String {
        crate::runtime::plugin_runtime_apply_state_status_json().await
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
