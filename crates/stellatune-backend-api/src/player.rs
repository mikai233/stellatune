use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::Receiver;

use crate::lyrics_service::LyricsService;
use crate::runtime::{
    init_tracing, register_plugin_runtime_engine, shared_plugins,
    subscribe_plugin_runtime_events_global,
};

use stellatune_audio::{EngineHandle, start_engine_with_plugins};
use stellatune_core::{
    AudioBackend, Command, DspChainItem, DspTypeDescriptor, Event, LyricsDoc, LyricsEvent,
    LyricsProviderTypeDescriptor, LyricsQuery, LyricsSearchCandidate, OutputSinkRoute,
    OutputSinkTypeDescriptor, PluginDescriptor, PluginRuntimeEvent, SourceCatalogTypeDescriptor,
    TrackDecodeInfo, TrackRef,
};

pub struct PlayerService {
    engine: EngineHandle,
    lyrics: Arc<LyricsService>,
}

impl PlayerService {
    pub fn new() -> Self {
        init_tracing();
        tracing::info!("creating player");
        let engine = start_engine_with_plugins(shared_plugins());
        register_plugin_runtime_engine(engine.clone());
        Self {
            engine,
            lyrics: LyricsService::new(),
        }
    }

    pub fn engine(&self) -> &EngineHandle {
        &self.engine
    }

    pub fn subscribe_events(&self) -> Receiver<Event> {
        self.engine.subscribe_events()
    }

    pub fn subscribe_plugin_runtime_events(&self) -> Receiver<PluginRuntimeEvent> {
        subscribe_plugin_runtime_events_global()
    }

    pub fn subscribe_lyrics_events(&self) -> Receiver<LyricsEvent> {
        self.lyrics.subscribe_events()
    }

    pub fn load(&self, path: String) {
        self.engine.send_command(Command::LoadTrackRef {
            track: TrackRef::for_local_path(path),
        });
    }

    pub fn load_track_ref(&self, track: TrackRef) {
        self.engine.send_command(Command::LoadTrackRef { track });
    }

    pub fn play(&self) {
        self.engine.send_command(Command::Play);
    }

    pub fn pause(&self) {
        self.engine.send_command(Command::Pause);
    }

    pub fn seek_ms(&self, position_ms: u64) {
        self.engine.send_command(Command::SeekMs { position_ms });
    }

    pub fn set_volume(&self, volume: f32) {
        self.engine.send_command(Command::SetVolume { volume });
    }

    pub fn stop(&self) {
        self.engine.send_command(Command::Stop);
    }

    pub fn lyrics_prepare(&self, query: LyricsQuery) -> Result<()> {
        self.lyrics.prepare(query)
    }

    pub fn lyrics_prefetch(&self, query: LyricsQuery) -> Result<()> {
        self.lyrics.prefetch(query)
    }

    pub async fn lyrics_search_candidates(
        &self,
        query: LyricsQuery,
    ) -> Result<Vec<LyricsSearchCandidate>> {
        self.lyrics.search_candidates(query).await
    }

    pub fn lyrics_apply_candidate(&self, track_key: String, doc: LyricsDoc) -> Result<()> {
        self.lyrics.apply_candidate(track_key, doc)
    }

    pub fn lyrics_set_cache_db_path(&self, db_path: String) -> Result<()> {
        self.lyrics.set_cache_db_path(db_path)
    }

    pub fn lyrics_clear_cache(&self) -> Result<()> {
        self.lyrics.clear_cache()
    }

    pub fn lyrics_refresh_current(&self) -> Result<()> {
        self.lyrics.refresh_current()
    }

    pub fn lyrics_set_position_ms(&self, position_ms: u64) {
        self.lyrics.set_position_ms(position_ms);
    }

    pub fn plugins_list(&self) -> Vec<PluginDescriptor> {
        self.engine.list_plugins()
    }

    pub fn plugin_publish_event_json(
        &self,
        plugin_id: Option<String>,
        event_json: String,
    ) -> Result<()> {
        self.engine
            .plugin_publish_event_json(plugin_id, event_json)
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn dsp_list_types(&self) -> Vec<DspTypeDescriptor> {
        self.engine.list_dsp_types()
    }

    pub fn source_list_types(&self) -> Vec<SourceCatalogTypeDescriptor> {
        self.engine.list_source_catalog_types()
    }

    pub fn lyrics_provider_list_types(&self) -> Vec<LyricsProviderTypeDescriptor> {
        self.engine.list_lyrics_provider_types()
    }

    pub fn output_sink_list_types(&self) -> Vec<OutputSinkTypeDescriptor> {
        self.engine.list_output_sink_types()
    }

    pub fn source_list_items_json(
        &self,
        plugin_id: String,
        type_id: String,
        config_json: String,
        request_json: String,
    ) -> Result<String> {
        let config = serde_json::from_str::<serde_json::Value>(&config_json)
            .map_err(|e| anyhow::anyhow!("invalid source config_json: {e}"))?;
        let request = serde_json::from_str::<serde_json::Value>(&request_json)
            .map_err(|e| anyhow::anyhow!("invalid source request_json: {e}"))?;
        let response: serde_json::Value = self
            .engine
            .source_list_items(&plugin_id, &type_id, &config, &request)
            .map_err(|e| anyhow::anyhow!(e))?;
        serde_json::to_string(&response)
            .map_err(|e| anyhow::anyhow!("serialize source list response: {e}"))
    }

    pub fn lyrics_provider_search_json(
        &self,
        plugin_id: String,
        type_id: String,
        query_json: String,
    ) -> Result<String> {
        let query = serde_json::from_str::<serde_json::Value>(&query_json)
            .map_err(|e| anyhow::anyhow!("invalid lyrics query_json: {e}"))?;
        let response: serde_json::Value = self
            .engine
            .lyrics_provider_search(&plugin_id, &type_id, &query)
            .map_err(|e| anyhow::anyhow!(e))?;
        serde_json::to_string(&response)
            .map_err(|e| anyhow::anyhow!("serialize lyrics search response: {e}"))
    }

    pub fn lyrics_provider_fetch_json(
        &self,
        plugin_id: String,
        type_id: String,
        track_json: String,
    ) -> Result<String> {
        let track = serde_json::from_str::<serde_json::Value>(&track_json)
            .map_err(|e| anyhow::anyhow!("invalid lyrics track_json: {e}"))?;
        let response: serde_json::Value = self
            .engine
            .lyrics_provider_fetch(&plugin_id, &type_id, &track)
            .map_err(|e| anyhow::anyhow!(e))?;
        serde_json::to_string(&response)
            .map_err(|e| anyhow::anyhow!("serialize lyrics fetch response: {e}"))
    }

    pub fn output_sink_list_targets_json(
        &self,
        plugin_id: String,
        type_id: String,
        config_json: String,
    ) -> Result<String> {
        let config = serde_json::from_str::<serde_json::Value>(&config_json)
            .map_err(|e| anyhow::anyhow!("invalid output sink config_json: {e}"))?;
        let response: serde_json::Value = self
            .engine
            .output_sink_list_targets(&plugin_id, &type_id, &config)
            .map_err(|e| anyhow::anyhow!(e))?;
        serde_json::to_string(&response)
            .map_err(|e| anyhow::anyhow!("serialize output sink targets: {e}"))
    }

    pub fn dsp_set_chain(&self, chain: Vec<DspChainItem>) {
        self.engine.set_dsp_chain(chain);
    }

    pub fn current_track_info(&self) -> Option<TrackDecodeInfo> {
        self.engine.current_track_info()
    }

    pub fn plugins_reload(&self, dir: String) {
        self.engine.reload_plugins(dir);
    }

    pub fn plugins_reload_with_disabled(&self, dir: String, disabled_ids: Vec<String>) {
        self.engine.reload_plugins_with_disabled(dir, disabled_ids);
    }

    pub fn refresh_devices(&self) {
        self.engine.send_command(Command::RefreshDevices);
    }

    pub fn set_output_device(&self, backend: AudioBackend, device_id: Option<String>) {
        self.engine
            .send_command(Command::SetOutputDevice { backend, device_id });
    }

    pub fn set_output_options(
        &self,
        match_track_sample_rate: bool,
        gapless_playback: bool,
        seek_track_fade: bool,
    ) {
        self.engine.send_command(Command::SetOutputOptions {
            match_track_sample_rate,
            gapless_playback,
            seek_track_fade,
        });
    }

    pub fn set_output_sink_route(&self, route: OutputSinkRoute) {
        self.engine
            .send_command(Command::SetOutputSinkRoute { route });
    }

    pub fn clear_output_sink_route(&self) {
        self.engine.send_command(Command::ClearOutputSinkRoute);
    }

    pub fn preload_track(&self, path: String, position_ms: u64) {
        self.engine.send_command(Command::PreloadTrackRef {
            track: TrackRef::for_local_path(path),
            position_ms,
        });
    }

    pub fn preload_track_ref(&self, track: TrackRef, position_ms: u64) {
        self.engine
            .send_command(Command::PreloadTrackRef { track, position_ms });
    }
}

impl Default for PlayerService {
    fn default() -> Self {
        Self::new()
    }
}

pub fn plugins_install_from_file(plugins_dir: String, artifact_path: String) -> Result<String> {
    let installed = stellatune_plugins::install_plugin_from_artifact(&plugins_dir, &artifact_path)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(installed.id)
}

pub fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    let installed = stellatune_plugins::list_installed_plugins(&plugins_dir)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    serde_json::to_string(&installed).map_err(|e| anyhow::anyhow!(e.to_string()))
}

pub fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    stellatune_plugins::uninstall_plugin(&plugins_dir, &plugin_id)
        .map_err(|e| anyhow::anyhow!(e.to_string()))
}
