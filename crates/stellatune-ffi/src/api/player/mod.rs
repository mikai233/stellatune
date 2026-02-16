use std::sync::{Arc, OnceLock};

use crate::frb_generated::StreamSink;
use anyhow::{Result, anyhow};
use stellatune_runtime as global_runtime;
use tracing::debug;

use stellatune_audio::{
    AudioBackend, AudioDevice, DspChainItem, DspTypeDescriptor, EngineHandle, Event,
    LyricsProviderTypeDescriptor, OutputSinkRoute, OutputSinkTypeDescriptor, PluginDescriptor,
    PluginRuntimeEvent, ResampleQuality, SourceCatalogTypeDescriptor, TrackDecodeInfo,
    TrackPlayability, TrackRef,
};
use stellatune_backend_api::lyrics_service::LyricsService;
use stellatune_backend_api::player::{
    plugins_install_from_file as backend_plugins_install_from_file,
    plugins_list_installed_json as backend_plugins_list_installed_json,
    plugins_uninstall_by_id as backend_plugins_uninstall_by_id,
};
use stellatune_backend_api::runtime::{shared_plugin_runtime, shared_runtime_engine};
use stellatune_backend_api::{LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate};
use stellatune_plugins::runtime::introspection::CapabilityKind;
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;

struct PlayerContext {
    engine: Arc<EngineHandle>,
    lyrics: Arc<LyricsService>,
}

fn shared_player_context() -> &'static PlayerContext {
    static CONTEXT: OnceLock<PlayerContext> = OnceLock::new();
    CONTEXT.get_or_init(|| PlayerContext {
        engine: shared_runtime_engine(),
        lyrics: LyricsService::new(),
    })
}

fn engine() -> Arc<EngineHandle> {
    Arc::clone(&shared_player_context().engine)
}

fn lyrics() -> Arc<LyricsService> {
    Arc::clone(&shared_player_context().lyrics)
}

pub async fn switch_track_ref(track: TrackRef, lazy: bool) -> Result<()> {
    engine()
        .switch_track_ref(track, lazy)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn play() -> Result<()> {
    engine().play().await.map_err(anyhow::Error::msg)
}

pub async fn pause() -> Result<()> {
    engine().pause().await.map_err(anyhow::Error::msg)
}

pub async fn seek_ms(position_ms: u64) -> Result<()> {
    engine()
        .seek_ms(position_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_volume(volume: f32) -> Result<()> {
    engine()
        .set_volume(volume)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn stop() -> Result<()> {
    engine().stop().await.map_err(anyhow::Error::msg)
}

pub fn events(sink: StreamSink<Event>) -> Result<()> {
    let mut rx = engine().subscribe_events();
    global_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if sink.add(event).is_err() {
                        debug!("events stream sink closed");
                        break;
                    }
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    debug!(skipped, "events lagged");
                },
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    Ok(())
}

pub fn plugin_runtime_events_global(sink: StreamSink<PluginRuntimeEvent>) -> Result<()> {
    let _ = sink;
    Err(anyhow!(
        "plugin runtime host-event channel has been removed"
    ))
}

pub async fn lyrics_prepare(query: LyricsQuery) -> Result<()> {
    lyrics().prepare(query).await
}

pub async fn lyrics_prefetch(query: LyricsQuery) -> Result<()> {
    lyrics().prefetch(query).await
}

pub async fn lyrics_search_candidates(query: LyricsQuery) -> Result<Vec<LyricsSearchCandidate>> {
    lyrics().search_candidates(query).await
}

pub async fn lyrics_apply_candidate(track_key: String, doc: LyricsDoc) -> Result<()> {
    lyrics().apply_candidate(track_key, doc).await
}

pub async fn lyrics_set_cache_db_path(db_path: String) -> Result<()> {
    lyrics().set_cache_db_path(db_path).await
}

pub async fn lyrics_clear_cache() -> Result<()> {
    lyrics().clear_cache().await
}

pub async fn lyrics_refresh_current() -> Result<()> {
    lyrics().refresh_current().await
}

pub fn lyrics_set_position_ms(position_ms: u64) {
    lyrics().set_position_ms(position_ms);
}

pub fn lyrics_events(sink: StreamSink<LyricsEvent>) -> Result<()> {
    let mut rx = lyrics().subscribe_events();
    global_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if sink.add(event).is_err() {
                        debug!("lyrics_events stream sink closed");
                        break;
                    }
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    debug!(skipped, "lyrics_events lagged");
                },
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    Ok(())
}

pub async fn plugins_list() -> Vec<PluginDescriptor> {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.active_plugin_ids().await;
    plugin_ids.sort();
    let mut out = Vec::with_capacity(plugin_ids.len());
    for plugin_id in plugin_ids {
        let Some(generation) = service.current_plugin_lease_info(&plugin_id).await else {
            continue;
        };
        out.push(PluginDescriptor {
            id: plugin_id.clone(),
            name: plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json),
        });
    }
    out
}

pub fn plugin_publish_event_json(plugin_id: Option<String>, event_json: String) -> Result<()> {
    let _ = (plugin_id, event_json);
    Err(anyhow!(
        "plugin runtime host-event channel has been removed"
    ))
}

pub async fn dsp_list_types() -> Vec<DspTypeDescriptor> {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.active_plugin_ids().await;
    plugin_ids.sort();
    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let Some(generation) = service.current_plugin_lease_info(&plugin_id).await else {
            continue;
        };
        let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
        let mut capabilities = service.list_capabilities(&plugin_id).await;
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            if capability.kind != CapabilityKind::Dsp {
                continue;
            }
            out.push(DspTypeDescriptor {
                plugin_id: plugin_id.clone(),
                plugin_name: plugin_name.clone(),
                type_id: capability.type_id,
                display_name: capability.display_name,
                config_schema_json: capability.config_schema_json,
                default_config_json: capability.default_config_json,
            });
        }
    }
    out
}

pub async fn source_list_types() -> Vec<SourceCatalogTypeDescriptor> {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.active_plugin_ids().await;
    plugin_ids.sort();
    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let Some(generation) = service.current_plugin_lease_info(&plugin_id).await else {
            continue;
        };
        let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
        let mut capabilities = service.list_capabilities(&plugin_id).await;
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            if capability.kind != CapabilityKind::SourceCatalog {
                continue;
            }
            out.push(SourceCatalogTypeDescriptor {
                plugin_id: plugin_id.clone(),
                plugin_name: plugin_name.clone(),
                type_id: capability.type_id,
                display_name: capability.display_name,
                config_schema_json: capability.config_schema_json,
                default_config_json: capability.default_config_json,
            });
        }
    }
    out
}

pub async fn lyrics_provider_list_types() -> Vec<LyricsProviderTypeDescriptor> {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.active_plugin_ids().await;
    plugin_ids.sort();
    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let Some(generation) = service.current_plugin_lease_info(&plugin_id).await else {
            continue;
        };
        let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
        let mut capabilities = service.list_capabilities(&plugin_id).await;
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            if capability.kind != CapabilityKind::LyricsProvider {
                continue;
            }
            out.push(LyricsProviderTypeDescriptor {
                plugin_id: plugin_id.clone(),
                plugin_name: plugin_name.clone(),
                type_id: capability.type_id,
                display_name: capability.display_name,
            });
        }
    }
    out
}

pub async fn output_sink_list_types() -> Vec<OutputSinkTypeDescriptor> {
    let service = shared_plugin_runtime();
    let mut plugin_ids = service.active_plugin_ids().await;
    plugin_ids.sort();
    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let Some(generation) = service.current_plugin_lease_info(&plugin_id).await else {
            continue;
        };
        let plugin_name = plugin_name_from_metadata_json(&plugin_id, &generation.metadata_json);
        let mut capabilities = service.list_capabilities(&plugin_id).await;
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            if capability.kind != CapabilityKind::OutputSink {
                continue;
            }
            out.push(OutputSinkTypeDescriptor {
                plugin_id: plugin_id.clone(),
                plugin_name: plugin_name.clone(),
                type_id: capability.type_id,
                display_name: capability.display_name,
                config_schema_json: capability.config_schema_json,
                default_config_json: capability.default_config_json,
            });
        }
    }
    out
}

pub async fn source_list_items_json(
    plugin_id: String,
    type_id: String,
    config_json: String,
    request_json: String,
) -> Result<String> {
    let config = serde_json::from_str::<serde_json::Value>(&config_json)
        .map_err(|e| anyhow!("invalid source config_json: {e}"))?;
    let request = serde_json::from_str::<serde_json::Value>(&request_json)
        .map_err(|e| anyhow!("invalid source request_json: {e}"))?;
    let endpoint = shared_plugin_runtime()
        .bind_source_catalog_worker_endpoint(&plugin_id, &type_id)
        .await
        .map_err(|e| anyhow!("bind source endpoint failed: {e}"))?;
    let (mut controller, _control_rx) = endpoint.into_controller(
        serde_json::to_string(&config).map_err(|e| anyhow!("serialize source config_json: {e}"))?,
    );
    match controller
        .apply_pending()
        .map_err(|e| anyhow!(e.to_string()))?
    {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(anyhow!("source controller did not create instance"));
        },
    }
    let instance = controller
        .instance_mut()
        .ok_or_else(|| anyhow!("source instance unavailable"))?;
    let payload = instance
        .list_items_json(
            &serde_json::to_string(&request)
                .map_err(|e| anyhow!("serialize source request_json: {e}"))?,
        )
        .await
        .map_err(|e| anyhow!("source list_items failed: {e}"))?;
    normalize_json_string_payload("source list response", payload)
}

pub async fn lyrics_provider_search_json(
    plugin_id: String,
    type_id: String,
    query_json: String,
) -> Result<String> {
    let query = serde_json::from_str::<serde_json::Value>(&query_json)
        .map_err(|e| anyhow!("invalid lyrics query_json: {e}"))?;
    let endpoint = shared_plugin_runtime()
        .bind_lyrics_provider_worker_endpoint(&plugin_id, &type_id)
        .await
        .map_err(|e| anyhow!("bind lyrics endpoint failed: {e}"))?;
    let (mut controller, _control_rx) = endpoint.into_controller("{}".to_string());
    match controller
        .apply_pending()
        .map_err(|e| anyhow!(e.to_string()))?
    {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(anyhow!("lyrics controller did not create instance"));
        },
    }
    let instance = controller
        .instance_mut()
        .ok_or_else(|| anyhow!("lyrics instance unavailable"))?;
    let payload = instance
        .search_json(
            &serde_json::to_string(&query)
                .map_err(|e| anyhow!("serialize lyrics query_json: {e}"))?,
        )
        .map_err(|e| anyhow!("lyrics search failed: {e}"))?;
    normalize_json_string_payload("lyrics search response", payload)
}

pub async fn lyrics_provider_fetch_json(
    plugin_id: String,
    type_id: String,
    track_json: String,
) -> Result<String> {
    let track = serde_json::from_str::<serde_json::Value>(&track_json)
        .map_err(|e| anyhow!("invalid lyrics track_json: {e}"))?;
    let endpoint = shared_plugin_runtime()
        .bind_lyrics_provider_worker_endpoint(&plugin_id, &type_id)
        .await
        .map_err(|e| anyhow!("bind lyrics endpoint failed: {e}"))?;
    let (mut controller, _control_rx) = endpoint.into_controller("{}".to_string());
    match controller
        .apply_pending()
        .map_err(|e| anyhow!(e.to_string()))?
    {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(anyhow!("lyrics controller did not create instance"));
        },
    }
    let instance = controller
        .instance_mut()
        .ok_or_else(|| anyhow!("lyrics instance unavailable"))?;
    let payload = instance
        .fetch_json(
            &serde_json::to_string(&track)
                .map_err(|e| anyhow!("serialize lyrics track_json: {e}"))?,
        )
        .map_err(|e| anyhow!("lyrics fetch failed: {e}"))?;
    normalize_json_string_payload("lyrics fetch response", payload)
}

pub async fn output_sink_list_targets_json(
    plugin_id: String,
    type_id: String,
    config_json: String,
) -> Result<String> {
    let config = serde_json::from_str::<serde_json::Value>(&config_json)
        .map_err(|e| anyhow!("invalid output sink config_json: {e}"))?;
    let endpoint = shared_plugin_runtime()
        .bind_output_sink_worker_endpoint(&plugin_id, &type_id)
        .await
        .map_err(|e| anyhow!("bind output sink endpoint failed: {e}"))?;
    let (mut controller, _control_rx) = endpoint.into_controller(
        serde_json::to_string(&config)
            .map_err(|e| anyhow!("serialize output sink config_json: {e}"))?,
    );
    match controller
        .apply_pending()
        .map_err(|e| anyhow!(e.to_string()))?
    {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(anyhow!("output sink controller did not create instance"));
        },
    }
    let instance = controller
        .instance_mut()
        .ok_or_else(|| anyhow!("output sink instance unavailable"))?;
    let payload = instance
        .list_targets_json()
        .map_err(|e| anyhow!("output sink list_targets failed: {e}"))?;
    normalize_json_string_payload("output sink targets", payload)
}

pub fn dsp_set_chain(chain: Vec<DspChainItem>) {
    engine().set_dsp_chain(chain);
}

pub fn current_track_info() -> Option<TrackDecodeInfo> {
    engine().current_track_info()
}

pub async fn plugins_install_from_file(
    plugins_dir: String,
    artifact_path: String,
) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        backend_plugins_install_from_file(plugins_dir, artifact_path)
    })
    .await
    .map_err(|e| anyhow!("JoinError: {e}"))?
}

pub async fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    tokio::task::spawn_blocking(move || backend_plugins_list_installed_json(plugins_dir))
        .await
        .map_err(|e| anyhow!("JoinError: {e}"))?
}

pub async fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    tokio::task::spawn_blocking(move || backend_plugins_uninstall_by_id(plugins_dir, plugin_id))
        .await
        .map_err(|e| anyhow!("JoinError: {e}"))?
}

pub async fn refresh_devices() -> Result<Vec<AudioDevice>> {
    engine().refresh_devices().await.map_err(anyhow::Error::msg)
}

pub async fn set_output_device(backend: AudioBackend, device_id: Option<String>) -> Result<()> {
    engine()
        .set_output_device(backend, device_id)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_output_options(
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
    resample_quality: ResampleQuality,
) -> Result<()> {
    engine()
        .set_output_options(
            match_track_sample_rate,
            gapless_playback,
            seek_track_fade,
            resample_quality,
        )
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_output_sink_route(route: OutputSinkRoute) -> Result<()> {
    engine()
        .set_output_sink_route(route)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn clear_output_sink_route() -> Result<()> {
    engine()
        .clear_output_sink_route()
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn preload_track(path: String, position_ms: u64) -> Result<()> {
    engine()
        .preload_track_ref(TrackRef::for_local_path(path), position_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn preload_track_ref(track: TrackRef, position_ms: u64) -> Result<()> {
    engine()
        .preload_track_ref(track, position_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub fn can_play_track_refs(tracks: Vec<TrackRef>) -> Vec<TrackPlayability> {
    engine().can_play_track_refs(tracks)
}

fn normalize_json_payload(label: &str, payload: serde_json::Value) -> Result<String> {
    serde_json::to_string(&payload).map_err(|e| anyhow!("serialize {label}: {e}"))
}

fn normalize_json_string_payload(label: &str, payload: String) -> Result<String> {
    let value = serde_json::from_str::<serde_json::Value>(&payload)
        .map_err(|e| anyhow!("deserialize {label}: {e}"))?;
    normalize_json_payload(label, value)
}

fn plugin_name_from_metadata_json(plugin_id: &str, metadata_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(metadata_json)
        .ok()
        .and_then(|v| {
            v.get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| plugin_id.to_string())
}
