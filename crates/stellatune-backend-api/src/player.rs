use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Result, anyhow};
use crossbeam_channel::Receiver;
use stellatune_plugin_protocol::PluginMetadata;

use crate::lyrics_service::LyricsService;
use crate::runtime::{
    init_tracing, register_plugin_runtime_engine, shared_plugin_runtime_v2,
    subscribe_plugin_runtime_events_global,
};

use stellatune_audio::{EngineHandle, start_engine};
use stellatune_core::{
    AudioBackend, Command, DspChainItem, DspTypeDescriptor, Event, LyricsDoc, LyricsEvent,
    LyricsProviderTypeDescriptor, LyricsQuery, LyricsSearchCandidate, OutputSinkRoute,
    OutputSinkTypeDescriptor, PluginDescriptor, PluginRuntimeEvent, SourceCatalogTypeDescriptor,
    TrackDecodeInfo, TrackPlayability, TrackRef,
};
use stellatune_plugins::runtime::CapabilityKind;

pub struct PlayerService {
    instance_id: u64,
    engine: EngineHandle,
    lyrics: Arc<LyricsService>,
}

impl PlayerService {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let instance_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        init_tracing();
        tracing::info!(instance_id, "creating player");
        let engine = start_engine();
        register_plugin_runtime_engine(engine.clone());
        Self {
            instance_id,
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
        runtime_plugin_catalog().plugins
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
        runtime_plugin_catalog().dsp_types
    }

    pub fn source_list_types(&self) -> Vec<SourceCatalogTypeDescriptor> {
        runtime_plugin_catalog().source_catalog_types
    }

    pub fn lyrics_provider_list_types(&self) -> Vec<LyricsProviderTypeDescriptor> {
        runtime_plugin_catalog().lyrics_provider_types
    }

    pub fn output_sink_list_types(&self) -> Vec<OutputSinkTypeDescriptor> {
        runtime_plugin_catalog().output_sink_types
    }

    pub fn source_list_items_json(
        &self,
        plugin_id: String,
        type_id: String,
        config_json: String,
        request_json: String,
    ) -> Result<String> {
        let _config = serde_json::from_str::<serde_json::Value>(&config_json)
            .map_err(|e| anyhow::anyhow!("invalid source config_json: {e}"))?;
        let _request = serde_json::from_str::<serde_json::Value>(&request_json)
            .map_err(|e| anyhow::anyhow!("invalid source request_json: {e}"))?;
        let payload = with_runtime_service(|service| {
            let mut inst =
                service.create_source_catalog_instance(&plugin_id, &type_id, &config_json)?;
            inst.list_items_json(&request_json)
        })?;
        normalize_json_payload("source list response", payload)
    }

    pub fn lyrics_provider_search_json(
        &self,
        plugin_id: String,
        type_id: String,
        query_json: String,
    ) -> Result<String> {
        let _query = serde_json::from_str::<serde_json::Value>(&query_json)
            .map_err(|e| anyhow::anyhow!("invalid lyrics query_json: {e}"))?;
        let payload = with_runtime_service(|service| {
            let config_json = capability_default_config_json(
                service,
                &plugin_id,
                CapabilityKind::LyricsProvider,
                &type_id,
            )?;
            let mut inst =
                service.create_lyrics_provider_instance(&plugin_id, &type_id, &config_json)?;
            inst.search_json(&query_json)
        })?;
        normalize_json_payload("lyrics search response", payload)
    }

    pub fn lyrics_provider_fetch_json(
        &self,
        plugin_id: String,
        type_id: String,
        track_json: String,
    ) -> Result<String> {
        let _track = serde_json::from_str::<serde_json::Value>(&track_json)
            .map_err(|e| anyhow::anyhow!("invalid lyrics track_json: {e}"))?;
        let payload = with_runtime_service(|service| {
            let config_json = capability_default_config_json(
                service,
                &plugin_id,
                CapabilityKind::LyricsProvider,
                &type_id,
            )?;
            let mut inst =
                service.create_lyrics_provider_instance(&plugin_id, &type_id, &config_json)?;
            inst.fetch_json(&track_json)
        })?;
        normalize_json_payload("lyrics fetch response", payload)
    }

    pub fn output_sink_list_targets_json(
        &self,
        plugin_id: String,
        type_id: String,
        config_json: String,
    ) -> Result<String> {
        let _config = serde_json::from_str::<serde_json::Value>(&config_json)
            .map_err(|e| anyhow::anyhow!("invalid output sink config_json: {e}"))?;
        let payload = with_runtime_service(|service| {
            let mut inst =
                service.create_output_sink_instance(&plugin_id, &type_id, &config_json)?;
            inst.list_targets_json()
        })?;
        normalize_json_payload("output sink targets", payload)
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

    pub fn can_play_track_refs(&self, tracks: Vec<TrackRef>) -> Vec<TrackPlayability> {
        self.engine.can_play_track_refs(tracks)
    }
}

impl Default for PlayerService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PlayerService {
    fn drop(&mut self) {
        tracing::info!(instance_id = self.instance_id, "dropping player");
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

#[derive(Default)]
struct RuntimePluginCatalog {
    plugins: Vec<PluginDescriptor>,
    dsp_types: Vec<DspTypeDescriptor>,
    source_catalog_types: Vec<SourceCatalogTypeDescriptor>,
    lyrics_provider_types: Vec<LyricsProviderTypeDescriptor>,
    output_sink_types: Vec<OutputSinkTypeDescriptor>,
}

fn runtime_plugin_catalog() -> RuntimePluginCatalog {
    let shared = shared_plugin_runtime_v2();
    let Ok(service) = shared.lock() else {
        return RuntimePluginCatalog::default();
    };

    let mut plugin_ids = service.active_plugin_ids();
    plugin_ids.sort();

    let mut catalog = RuntimePluginCatalog::default();
    for plugin_id in plugin_ids {
        let Some(generation) = service.active_generation(&plugin_id) else {
            continue;
        };
        let plugin_name = parse_plugin_name_from_metadata(&plugin_id, &generation.metadata_json);
        catalog.plugins.push(PluginDescriptor {
            id: plugin_id.clone(),
            name: plugin_name.clone(),
        });

        let mut capabilities = service.list_active_capabilities(&plugin_id);
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            match capability.kind {
                CapabilityKind::Decoder => {}
                CapabilityKind::Dsp => catalog.dsp_types.push(DspTypeDescriptor {
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id: capability.type_id,
                    display_name: capability.display_name,
                    config_schema_json: capability.config_schema_json,
                    default_config_json: capability.default_config_json,
                }),
                CapabilityKind::SourceCatalog => {
                    catalog
                        .source_catalog_types
                        .push(SourceCatalogTypeDescriptor {
                            plugin_id: plugin_id.clone(),
                            plugin_name: plugin_name.clone(),
                            type_id: capability.type_id,
                            display_name: capability.display_name,
                            config_schema_json: capability.config_schema_json,
                            default_config_json: capability.default_config_json,
                        })
                }
                CapabilityKind::LyricsProvider => {
                    catalog
                        .lyrics_provider_types
                        .push(LyricsProviderTypeDescriptor {
                            plugin_id: plugin_id.clone(),
                            plugin_name: plugin_name.clone(),
                            type_id: capability.type_id,
                            display_name: capability.display_name,
                        })
                }
                CapabilityKind::OutputSink => {
                    catalog.output_sink_types.push(OutputSinkTypeDescriptor {
                        plugin_id: plugin_id.clone(),
                        plugin_name: plugin_name.clone(),
                        type_id: capability.type_id,
                        display_name: capability.display_name,
                        config_schema_json: capability.config_schema_json,
                        default_config_json: capability.default_config_json,
                    })
                }
            }
        }
    }

    catalog
}

fn parse_plugin_name_from_metadata(plugin_id: &str, metadata_json: &str) -> String {
    serde_json::from_str::<PluginMetadata>(metadata_json)
        .map(|v| v.name)
        .unwrap_or_else(|_| plugin_id.to_string())
}

fn with_runtime_service<T>(
    f: impl FnOnce(&stellatune_plugins::v2::PluginRuntimeService) -> Result<T>,
) -> Result<T> {
    let shared = shared_plugin_runtime_v2();
    let service = shared
        .lock()
        .map_err(|_| anyhow!("plugin runtime v2 mutex poisoned"))?;
    f(&service)
}

fn capability_default_config_json(
    service: &stellatune_plugins::v2::PluginRuntimeService,
    plugin_id: &str,
    kind: CapabilityKind,
    type_id: &str,
) -> Result<String> {
    service
        .resolve_active_capability(plugin_id, kind, type_id)
        .map(|c| c.default_config_json)
        .ok_or_else(|| anyhow!("capability not found: {plugin_id}::{type_id}"))
}

fn normalize_json_payload(label: &str, payload: String) -> Result<String> {
    let value = serde_json::from_str::<serde_json::Value>(&payload)
        .map_err(|e| anyhow!("invalid {label}: {e}"))?;
    serde_json::to_string(&value).map_err(|e| anyhow!("serialize {label}: {e}"))
}
