use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use tokio::sync::broadcast;

use stellatune_audio::config::engine::{
    EngineSnapshot, Event as AudioEvent, PlayerState, ResampleQuality,
};
use stellatune_audio::engine::EngineHandle;
use stellatune_audio::pipeline::assembly::{BuiltinTransformSlot, PipelineMutation};
use stellatune_backend_api::app::BackendApp;
use stellatune_backend_api::library::LibraryService;
use stellatune_backend_api::runtime::runtime_set_output_options;
use stellatune_backend_api::session::{BackendSession, BackendSessionOptions};
use stellatune_library::{LibraryEvent, PlaylistLite, TrackLite};

use super::models::InstalledPluginInfo;
use super::track_token::encode_local_track_token;

pub struct BackendFacade {
    app: BackendApp,
    session: BackendSession,
    plugins_dir: PathBuf,
    page_size: i64,
}

impl BackendFacade {
    pub async fn new(db_path: &Path, plugins_dir: &Path, page_size: i64) -> Result<Self> {
        let app = BackendApp::new();
        let session = app
            .create_session(BackendSessionOptions::with_library(
                db_path.to_string_lossy().to_string(),
            ))
            .await?;

        Ok(Self {
            app,
            session,
            plugins_dir: plugins_dir.to_path_buf(),
            page_size: page_size.max(1),
        })
    }

    pub fn player(&self) -> &Arc<EngineHandle> {
        self.session.player()
    }

    fn library(&self) -> Result<&LibraryService> {
        self.session
            .library()
            .ok_or_else(|| anyhow!("library session is not attached"))
    }

    pub fn subscribe_player_events(&self) -> broadcast::Receiver<AudioEvent> {
        self.player().subscribe_events()
    }

    pub fn subscribe_library_events(&self) -> Result<broadcast::Receiver<LibraryEvent>> {
        Ok(self.library()?.subscribe_events())
    }

    pub async fn snapshot(&self) -> Result<EngineSnapshot> {
        self.player()
            .snapshot()
            .await
            .map_err(|e| anyhow!("snapshot failed: {e}"))
    }

    pub async fn play(&self) -> Result<()> {
        self.player()
            .play()
            .await
            .map_err(|e| anyhow!("play failed: {e}"))
    }

    pub async fn pause(&self) -> Result<()> {
        self.player()
            .pause()
            .await
            .map_err(|e| anyhow!("pause failed: {e}"))
    }

    pub async fn stop(&self) -> Result<()> {
        self.player()
            .stop()
            .await
            .map_err(|e| anyhow!("stop failed: {e}"))
    }

    pub async fn toggle_play_pause(&self) -> Result<()> {
        let snapshot = self.snapshot().await?;
        match snapshot.state {
            PlayerState::Playing => self.pause().await,
            PlayerState::Paused | PlayerState::Stopped => self.play().await,
        }
    }

    pub async fn seek_ms(&self, position_ms: i64) -> Result<()> {
        self.player()
            .seek_ms(position_ms.max(0))
            .await
            .map_err(|e| anyhow!("seek_ms failed: {e}"))
    }

    pub async fn set_audio_output_settings(
        &self,
        match_track_sample_rate: bool,
        resample_quality: ResampleQuality,
        gapless_playback: bool,
        seek_track_fade: bool,
    ) -> Result<()> {
        self.player()
            .set_resample_quality(resample_quality)
            .await
            .map_err(|e| anyhow!("set_resample_quality failed: {e}"))?;
        runtime_set_output_options(match_track_sample_rate, resample_quality)
            .await
            .map_err(|e| anyhow!("runtime_set_output_options failed: {e}"))?;
        self.player()
            .apply_pipeline_mutation(PipelineMutation::SetBuiltinTransformSlot {
                slot: BuiltinTransformSlot::GaplessTrim,
                enabled: gapless_playback,
            })
            .await
            .map_err(|e| anyhow!("set gapless slot failed: {e}"))?;
        self.player()
            .apply_pipeline_mutation(PipelineMutation::SetBuiltinTransformSlot {
                slot: BuiltinTransformSlot::TransitionGain,
                enabled: seek_track_fade,
            })
            .await
            .map_err(|e| anyhow!("set seek-fade slot failed: {e}"))?;
        Ok(())
    }

    pub async fn play_track_path(&self, path: &str) -> Result<()> {
        let token = encode_local_track_token(path);
        self.player()
            .switch_track_token(token, true)
            .await
            .map_err(|e| anyhow!("switch_track_token failed: {e}"))
    }

    pub async fn add_root(&self, path: String) -> Result<()> {
        self.library()?.add_root(path).await
    }

    pub async fn remove_root(&self, path: String) -> Result<()> {
        self.library()?.remove_root(path).await
    }

    pub async fn scan_all(&self, force: bool) -> Result<()> {
        if force {
            self.library()?.scan_all_force().await
        } else {
            self.library()?.scan_all().await
        }
    }

    pub async fn list_roots(&self) -> Result<Vec<String>> {
        self.library()?.list_roots().await
    }

    pub async fn list_tracks(&self, folder: String, query: String) -> Result<Vec<TrackLite>> {
        self.library()?
            .list_tracks(folder, true, query, self.page_size, 0)
            .await
    }

    pub async fn search_tracks(&self, query: String) -> Result<Vec<TrackLite>> {
        self.library()?.search(query, self.page_size, 0).await
    }

    pub async fn list_playlists(&self) -> Result<Vec<PlaylistLite>> {
        self.library()?.list_playlists().await
    }

    pub async fn list_playlist_tracks(
        &self,
        playlist_id: i64,
        query: String,
    ) -> Result<Vec<TrackLite>> {
        self.library()?
            .list_playlist_tracks(playlist_id, query, self.page_size, 0)
            .await
    }

    pub async fn create_playlist(&self, name: String) -> Result<()> {
        self.library()?.create_playlist(name).await
    }

    pub async fn rename_playlist(&self, id: i64, name: String) -> Result<()> {
        self.library()?.rename_playlist(id, name).await
    }

    pub async fn delete_playlist(&self, id: i64) -> Result<()> {
        self.library()?.delete_playlist(id).await
    }

    pub async fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.library()?
            .add_track_to_playlist(playlist_id, track_id)
            .await
    }

    pub async fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.library()?
            .remove_track_from_playlist(playlist_id, track_id)
            .await
    }

    pub async fn plugins_list_installed(&self) -> Result<Vec<InstalledPluginInfo>> {
        let raw = self
            .app
            .plugins_list_installed_json(self.plugins_dir.to_string_lossy().to_string())?;
        let mut list: Vec<InstalledPluginInfo> = serde_json::from_str(&raw)?;
        list.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(list)
    }

    pub async fn plugins_install_from_file(&self, artifact_path: String) -> Result<String> {
        self.app.plugins_install_from_file(
            self.plugins_dir.to_string_lossy().to_string(),
            artifact_path,
        )
    }

    pub async fn plugins_uninstall_by_id(&self, plugin_id: String) -> Result<()> {
        self.app
            .plugins_uninstall_by_id(self.plugins_dir.to_string_lossy().to_string(), plugin_id)
    }

    pub async fn plugin_enable(&self, plugin_id: String) -> Result<()> {
        self.library()?.plugin_enable(plugin_id).await
    }

    pub async fn plugin_disable(&self, plugin_id: String) -> Result<()> {
        self.library()?.plugin_disable(plugin_id).await
    }

    pub async fn plugin_apply_state(&self) -> Result<()> {
        self.library()?.plugin_apply_state().await
    }

    pub async fn list_disabled_plugin_ids(&self) -> Result<HashSet<String>> {
        let ids = self.library()?.list_disabled_plugin_ids().await?;
        Ok(ids.into_iter().collect())
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    pub async fn active_plugin_ids(&self) -> Vec<String> {
        let mut ids = stellatune_backend_api::runtime::shared_plugin_runtime()
            .active_plugin_ids()
            .await;
        ids.sort();
        ids
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    pub async fn active_plugin_ids(&self) -> Vec<String> {
        Vec::new()
    }
}
