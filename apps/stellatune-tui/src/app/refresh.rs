use std::collections::HashSet;

use anyhow::Result;

use super::state::clamp_selection;
use super::{App, track_paths_match};

impl App {
    pub(super) async fn refresh_all(&mut self) -> Result<()> {
        self.refresh_library().await?;
        self.refresh_playlists().await?;
        self.refresh_plugins().await?;
        Ok(())
    }

    pub(super) async fn refresh_library(&mut self) -> Result<()> {
        self.state.library.roots = self.backend.list_roots().await?;
        clamp_selection(
            &mut self.state.library.selected_root,
            self.state.library.roots.len(),
        );
        self.refresh_library_tracks_only().await
    }

    pub(super) async fn refresh_library_tracks_only(&mut self) -> Result<()> {
        let folder = self.state.library.current_root();
        self.state.library.tracks = self.backend.list_tracks(folder, String::new()).await?;
        self.state.library.search_query = None;
        clamp_selection(
            &mut self.state.library.selected_track,
            self.state.library.tracks.len(),
        );
        self.refresh_playback_duration_hint();
        Ok(())
    }

    pub(super) async fn refresh_playlists(&mut self) -> Result<()> {
        self.state.playlists.playlists = self.backend.list_playlists().await?;
        clamp_selection(
            &mut self.state.playlists.selected_playlist,
            self.state.playlists.playlists.len(),
        );
        self.refresh_playlist_tracks().await
    }

    pub(super) async fn refresh_playlist_tracks(&mut self) -> Result<()> {
        let playlist_id = self
            .state
            .playlists
            .current_playlist()
            .map(|p| p.id)
            .unwrap_or_default();
        self.state.playlists.tracks = if playlist_id <= 0 {
            Vec::new()
        } else {
            self.backend
                .list_playlist_tracks(playlist_id, String::new())
                .await?
        };
        clamp_selection(
            &mut self.state.playlists.selected_track,
            self.state.playlists.tracks.len(),
        );
        self.refresh_playback_duration_hint();
        Ok(())
    }

    pub(super) async fn refresh_plugins(&mut self) -> Result<()> {
        self.state.plugins.installed = self.backend.plugins_list_installed().await?;
        self.state.plugins.disabled_ids = self.backend.list_disabled_plugin_ids().await?;
        self.state.plugins.active_ids =
            self.backend.active_plugin_ids().await.into_iter().collect();
        clamp_selection(
            &mut self.state.plugins.selected,
            self.state.plugins.installed.len(),
        );
        Ok(())
    }

    pub(super) fn try_action(&mut self, label: &str, result: Result<()>) {
        match result {
            Ok(()) => self.toast_info(format!("ok: {label}")),
            Err(error) => self.toast_error(format!("{label} failed: {error}")),
        }
    }

    pub(super) async fn play_track_with_hint(
        &mut self,
        path: String,
        duration_ms: Option<i64>,
    ) -> Result<()> {
        let library_track_id = self
            .lookup_track_id(&path)
            .ok_or_else(|| anyhow::anyhow!("track is not present in the local catalog: {path}"))?;
        let result = self.backend.play_library_track(library_track_id).await;
        if result.is_ok() {
            self.state.playback.current_track_display = path.clone();
            self.state.playback.position_ms = 0;
            self.state.playback.duration_ms = duration_ms
                .filter(|value| *value > 0)
                .or_else(|| self.lookup_track_duration_hint(&path));
            self.sync_queue_cursor_with_path(&path);
        }
        result
    }

    pub(super) fn refresh_playback_duration_hint(&mut self) {
        if self.state.playback.duration_ms.is_some() {
            return;
        }
        self.state.playback.duration_ms =
            self.lookup_track_duration_hint(&self.state.playback.current_track_display);
    }

    pub(super) fn lookup_track_duration_hint(&self, track_path: &str) -> Option<i64> {
        self.state
            .library
            .tracks
            .iter()
            .chain(self.state.playlists.tracks.iter())
            .find(|track| track_paths_match(&track.path, track_path))
            .and_then(|track| track.duration_ms.filter(|value| *value > 0))
    }

    pub(super) fn lookup_track_id(&self, track_path: &str) -> Option<i64> {
        self.state
            .library
            .tracks
            .iter()
            .chain(self.state.playlists.tracks.iter())
            .find(|track| track_paths_match(&track.path, track_path))
            .map(|track| track.id)
    }

    pub(super) fn sync_queue_cursor_with_path(&mut self, track_path: &str) {
        self.state.queue_index = self
            .state
            .queue
            .iter()
            .position(|item| track_paths_match(&item.path, track_path));
    }

    pub fn plugin_status(
        disabled_ids: &HashSet<String>,
        active_ids: &HashSet<String>,
        id: &str,
    ) -> &'static str {
        if disabled_ids.contains(id) {
            "disabled"
        } else if active_ids.contains(id) {
            "enabled"
        } else {
            "installed"
        }
    }
}
