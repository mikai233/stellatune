use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

use crate::frb_generated::StreamSink;
use anyhow::{Result, anyhow};
use serde_json::Value;
use stellatune_runtime as global_runtime;
use tracing::{debug, warn};

pub(crate) mod types;
use stellatune_audio_plugin_adapters::decoder_stage::probe_track_decode_info;
use stellatune_audio_plugin_adapters::orchestrator::PluginPipelineOrchestrator;
use stellatune_audio_plugin_adapters::v2_bridge::{
    PluginTransformSegment, PluginTransformStageSpec,
};
use stellatune_audio_v2::assembly::{BuiltinTransformSlot, PipelineMutation};
use stellatune_audio_v2::control::EngineHandle as V2EngineHandle;
use stellatune_audio_v2::types::{
    Event as V2Event, LfeMode as V2LfeMode, PlayerState as V2PlayerState,
    ResampleQuality as V2ResampleQuality,
};
use stellatune_backend_api::lyrics_service::LyricsService;
use stellatune_backend_api::player::{
    plugins_install_from_file as backend_plugins_install_from_file,
    plugins_list_installed_json as backend_plugins_list_installed_json,
    plugins_uninstall_by_id as backend_plugins_uninstall_by_id,
};
use stellatune_backend_api::runtime::{
    OutputBackend as RuntimeOutputBackend, runtime_clear_output_sink_route,
    runtime_list_output_devices, runtime_set_output_device, runtime_set_output_options,
    runtime_set_output_sink_route, shared_plugin_runtime, shared_runtime_engine,
};
use stellatune_backend_api::{LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate};
use stellatune_plugins::runtime::introspection::CapabilityKind;
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;
use types::{
    AudioBackend, AudioDevice, DspChainItem, DspTypeDescriptor, Event, LfeMode,
    LyricsProviderTypeDescriptor, OutputSinkRoute, OutputSinkTypeDescriptor, PlayerState,
    PluginDescriptor, PluginRuntimeEvent, ResampleQuality, SourceCatalogTypeDescriptor,
    TrackDecodeInfo, TrackPlayability, TrackRef,
};

struct PlayerContext {
    engine: Arc<V2EngineHandle>,
    lyrics: Arc<LyricsService>,
    dsp_orchestrator: Arc<Mutex<PluginPipelineOrchestrator>>,
    track_info_cache: Arc<Mutex<Option<CachedTrackDecodeInfo>>>,
    pending_preload_seek: Arc<Mutex<Option<PendingPreloadSeek>>>,
}

#[derive(Debug, Clone)]
struct CachedTrackDecodeInfo {
    track_token: String,
    info: Option<TrackDecodeInfo>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(ignore)]
struct PendingPreloadSeek {
    track_token: String,
    position_ms: i64,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(ignore)]
struct FfiEventMapperState {
    last_track_path: String,
    position_session_id: u64,
    recovering: bool,
    last_player_state: PlayerState,
}

impl Default for FfiEventMapperState {
    fn default() -> Self {
        Self {
            last_track_path: String::new(),
            position_session_id: 0,
            recovering: false,
            last_player_state: PlayerState::Stopped,
        }
    }
}

fn shared_player_context() -> &'static PlayerContext {
    static CONTEXT: OnceLock<PlayerContext> = OnceLock::new();
    CONTEXT.get_or_init(|| PlayerContext {
        engine: shared_runtime_engine(),
        lyrics: LyricsService::new(),
        dsp_orchestrator: Arc::new(Mutex::new(PluginPipelineOrchestrator::new())),
        track_info_cache: Arc::new(Mutex::new(None)),
        pending_preload_seek: Arc::new(Mutex::new(None)),
    })
}

fn engine() -> Arc<V2EngineHandle> {
    Arc::clone(&shared_player_context().engine)
}

fn lyrics() -> Arc<LyricsService> {
    Arc::clone(&shared_player_context().lyrics)
}

pub async fn switch_track_ref(track: TrackRef, lazy: bool) -> Result<()> {
    let autoplay = !lazy;
    let result = engine()
        .switch_track_token(encode_track_ref_token(&track), autoplay)
        .await
        .map_err(anyhow::Error::msg);
    if result.is_ok() {
        clear_cached_track_info();
        clear_pending_preload_seek();
    }
    result
}

pub async fn play() -> Result<()> {
    engine().play().await.map_err(anyhow::Error::msg)
}

pub async fn pause() -> Result<()> {
    engine().pause().await.map_err(anyhow::Error::msg)
}

pub async fn seek_ms(position_ms: u64) -> Result<()> {
    let position_ms = if position_ms > i64::MAX as u64 {
        i64::MAX
    } else {
        position_ms as i64
    };
    engine()
        .seek_ms(position_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_volume(volume: f32, seq: u64, ramp_ms: u32) -> Result<()> {
    engine()
        .set_volume(volume, seq, ramp_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_lfe_mode(mode: LfeMode) -> Result<()> {
    engine()
        .set_lfe_mode(map_lfe_mode(mode))
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn stop() -> Result<()> {
    let result = engine().stop().await.map_err(anyhow::Error::msg);
    if result.is_ok() {
        clear_cached_track_info();
        clear_pending_preload_seek();
    }
    result
}

pub fn events(sink: StreamSink<Event>) -> Result<()> {
    let mut rx = engine().subscribe_events();
    let event_engine = engine();
    let pending_preload_seek = Arc::clone(&shared_player_context().pending_preload_seek);
    global_runtime::spawn(async move {
        let mut state = FfiEventMapperState::default();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(position_ms) =
                        take_pending_preload_seek_for_event(&event, pending_preload_seek.as_ref())
                        && let Err(error) = event_engine.seek_ms(position_ms).await
                    {
                        warn!(
                            position_ms,
                            error, "failed to apply pending preload seek on track switch"
                        );
                    }
                    let mapped = map_v2_event_to_ffi(event, &mut state);
                    for mapped_event in mapped {
                        if sink.add(mapped_event).is_err() {
                            debug!("events stream sink closed");
                            return;
                        }
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

pub async fn dsp_set_chain(chain: Vec<DspChainItem>) {
    let specs: Vec<PluginTransformStageSpec> = chain
        .into_iter()
        .filter_map(|item| {
            let plugin_id = item.plugin_id.trim().to_string();
            let type_id = item.type_id.trim().to_string();
            if plugin_id.is_empty() || type_id.is_empty() {
                warn!("skip dsp chain item with empty plugin_id/type_id");
                return None;
            }
            Some(PluginTransformStageSpec {
                plugin_id,
                type_id,
                config_json: item.config_json,
                segment: PluginTransformSegment::Main,
            })
        })
        .collect();

    let engine = engine();
    let orchestrator = Arc::clone(&shared_player_context().dsp_orchestrator);
    let mut staged_orchestrator = match orchestrator.lock() {
        Ok(guard) => (*guard).clone(),
        Err(_) => {
            warn!("dsp orchestrator mutex poisoned");
            return;
        },
    };

    let mut planned_mutations = Vec::<PipelineMutation>::new();
    let build_result = staged_orchestrator.replace_transform_chain_filtered_with(
        &specs,
        &HashSet::new(),
        |mutation| {
            planned_mutations.push(mutation);
            Ok(())
        },
    );
    if let Err(error) = build_result {
        warn!(error, "failed to plan dsp chain transform mutations");
        return;
    }

    for mutation in planned_mutations.iter().cloned() {
        if let Err(error) = engine.apply_pipeline_mutation(mutation).await {
            warn!(
                error,
                "failed to apply dsp chain via v2 transform graph mutations"
            );
            return;
        }
    }

    match orchestrator.lock() {
        Ok(mut guard) => {
            *guard = staged_orchestrator;
        },
        Err(_) => {
            warn!("dsp orchestrator mutex poisoned when committing state");
        },
    }
}

pub async fn current_track_info() -> Option<TrackDecodeInfo> {
    let snapshot = match engine().snapshot().await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            warn!(error, "current_track_info snapshot failed");
            return None;
        },
    };
    let Some(track_token) = snapshot.current_track else {
        clear_cached_track_info();
        return None;
    };

    if let Ok(cache_guard) = shared_player_context().track_info_cache.lock()
        && let Some(entry) = cache_guard.as_ref()
        && entry.track_token == track_token
    {
        return entry.info.clone();
    }

    let info = match probe_track_decode_info(track_token.as_str()) {
        Ok(probed) => Some(TrackDecodeInfo {
            sample_rate: probed.sample_rate,
            channels: probed.channels,
            duration_ms: probed.duration_ms,
            metadata_json: probed.metadata_json,
            decoder_plugin_id: Some(probed.decoder_plugin_id),
            decoder_type_id: Some(probed.decoder_type_id),
        }),
        Err(error) => {
            warn!(track_token, error, "current_track_info probe failed");
            None
        },
    };

    if let Ok(mut cache_guard) = shared_player_context().track_info_cache.lock() {
        *cache_guard = Some(CachedTrackDecodeInfo {
            track_token,
            info: info.clone(),
        });
    }
    info
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
    let devices = runtime_list_output_devices().map_err(anyhow::Error::msg)?;
    Ok(devices
        .into_iter()
        .map(|device| AudioDevice {
            backend: match device.backend {
                RuntimeOutputBackend::Shared => AudioBackend::Shared,
                RuntimeOutputBackend::WasapiExclusive => AudioBackend::WasapiExclusive,
            },
            id: device.id,
            name: device.name,
        })
        .collect())
}

pub async fn set_output_device(backend: AudioBackend, device_id: Option<String>) -> Result<()> {
    let backend = match backend {
        AudioBackend::Shared => RuntimeOutputBackend::Shared,
        AudioBackend::WasapiExclusive => RuntimeOutputBackend::WasapiExclusive,
    };
    runtime_set_output_device(backend, device_id)
        .await
        .map(|_| ())
        .map_err(anyhow::Error::msg)
}

pub async fn set_output_options(
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
    resample_quality: ResampleQuality,
) -> Result<()> {
    let handle = engine();
    let mapped_quality = map_resample_quality(resample_quality);
    handle
        .set_resample_quality(mapped_quality)
        .await
        .map_err(anyhow::Error::msg)?;
    runtime_set_output_options(match_track_sample_rate, mapped_quality)
        .await
        .map_err(anyhow::Error::msg)?;
    handle
        .apply_pipeline_mutation(PipelineMutation::SetBuiltinTransformSlot {
            slot: BuiltinTransformSlot::GaplessTrim,
            enabled: gapless_playback,
        })
        .await
        .map_err(anyhow::Error::msg)?;
    handle
        .apply_pipeline_mutation(PipelineMutation::SetBuiltinTransformSlot {
            slot: BuiltinTransformSlot::TransitionGain,
            enabled: seek_track_fade,
        })
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_output_sink_route(route: OutputSinkRoute) -> Result<()> {
    let _target = route.target::<Value>().map_err(|e| {
        anyhow!(
            "invalid output sink route target_json for {}::{}: {e}",
            route.plugin_id,
            route.type_id
        )
    })?;

    runtime_set_output_sink_route(
        route.plugin_id,
        route.type_id,
        route.config_json,
        route.target_json,
    )
    .await
    .map_err(anyhow::Error::msg)
}

pub async fn clear_output_sink_route() -> Result<()> {
    runtime_clear_output_sink_route()
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn preload_track(path: String, position_ms: u64) -> Result<()> {
    preload_track_ref(TrackRef::for_local_path(path), position_ms).await
}

pub async fn preload_track_ref(track: TrackRef, position_ms: u64) -> Result<()> {
    let track_token = encode_track_ref_token(&track);
    engine()
        .queue_next_track_token(track_token.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    let seek_position_ms = position_ms.min(i64::MAX as u64) as i64;
    let pending_seek = if seek_position_ms > 0 {
        Some(PendingPreloadSeek {
            track_token,
            position_ms: seek_position_ms,
        })
    } else {
        None
    };
    set_pending_preload_seek(pending_seek);
    Ok(())
}

pub async fn decoder_supported_extensions() -> Vec<String> {
    let service = shared_plugin_runtime();
    let mut out = service.decoder_supported_extensions_cached();
    if service.decoder_has_wildcard_candidate_cached() {
        out.push("*".to_string());
    }
    out.sort();
    out.dedup();
    out
}

pub fn can_play_track_refs(tracks: Vec<TrackRef>) -> Vec<TrackPlayability> {
    tracks
        .into_iter()
        .map(|track| {
            let track_token = encode_track_ref_token(&track);
            match probe_track_decode_info(track_token.as_str()) {
                Ok(_) => TrackPlayability {
                    track,
                    playable: true,
                    reason: None,
                },
                Err(reason) => TrackPlayability {
                    track,
                    playable: false,
                    reason: Some(reason),
                },
            }
        })
        .collect()
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

fn clear_cached_track_info() {
    if let Ok(mut cache_guard) = shared_player_context().track_info_cache.lock() {
        *cache_guard = None;
    }
}

fn clear_pending_preload_seek() {
    set_pending_preload_seek(None);
}

fn set_pending_preload_seek(pending: Option<PendingPreloadSeek>) {
    if let Ok(mut guard) = shared_player_context().pending_preload_seek.lock() {
        *guard = pending;
    }
}

fn encode_track_ref_token(track: &TrackRef) -> String {
    serde_json::to_string(track).unwrap_or_else(|_| track.locator.clone())
}

fn decode_track_token_path(track_token: &str) -> String {
    serde_json::from_str::<TrackRef>(track_token)
        .map(|track| track.locator)
        .unwrap_or_else(|_| track_token.to_string())
}

fn take_pending_preload_seek_for_event(
    event: &V2Event,
    pending: &Mutex<Option<PendingPreloadSeek>>,
) -> Option<i64> {
    let V2Event::TrackChanged { track_token } = event else {
        return None;
    };
    let mut guard = pending.lock().ok()?;
    let pending_seek = guard.as_ref()?;
    if pending_seek.track_token != *track_token {
        return None;
    }
    let position_ms = pending_seek.position_ms;
    *guard = None;
    Some(position_ms)
}

fn next_position_session_id(current: &mut u64) -> u64 {
    *current = current.wrapping_add(1);
    if *current == 0 {
        *current = 1;
    }
    *current
}

fn map_v2_event_to_ffi(event: V2Event, state: &mut FfiEventMapperState) -> Vec<Event> {
    match event {
        V2Event::StateChanged { state: next_state } => {
            let mapped = map_player_state(next_state);
            state.last_player_state = mapped;
            state.recovering = false;
            vec![Event::StateChanged { state: mapped }]
        },
        V2Event::TrackChanged { track_token } => {
            let path = decode_track_token_path(&track_token);
            state.last_track_path = path.clone();
            let _ = next_position_session_id(&mut state.position_session_id);
            vec![Event::TrackChanged { path }]
        },
        V2Event::Recovering {
            attempt,
            backoff_ms,
        } => {
            let mut out = Vec::with_capacity(2);
            if !state.recovering {
                state.recovering = true;
                out.push(Event::StateChanged {
                    state: PlayerState::Buffering,
                });
            }
            out.push(Event::Log {
                message: format!(
                    "runtime recovering output stream (attempt={attempt}, backoff_ms={backoff_ms})"
                ),
            });
            out
        },
        V2Event::Position { position_ms } => {
            let mut out = Vec::with_capacity(2);
            if state.recovering {
                state.recovering = false;
                out.push(Event::StateChanged {
                    state: state.last_player_state,
                });
            }
            out.push(Event::Position {
                ms: position_ms,
                path: state.last_track_path.clone(),
                session_id: state.position_session_id,
            });
            out
        },
        V2Event::VolumeChanged { volume, seq } => vec![Event::VolumeChanged { volume, seq }],
        V2Event::Eof => {
            state.recovering = false;
            vec![Event::PlaybackEnded {
                path: state.last_track_path.clone(),
            }]
        },
        V2Event::Error { message } => {
            state.recovering = false;
            vec![Event::Error { message }]
        },
    }
}

fn map_player_state(state: V2PlayerState) -> PlayerState {
    match state {
        V2PlayerState::Stopped => PlayerState::Stopped,
        V2PlayerState::Paused => PlayerState::Paused,
        V2PlayerState::Playing => PlayerState::Playing,
    }
}

fn map_lfe_mode(mode: LfeMode) -> V2LfeMode {
    match mode {
        LfeMode::Mute => V2LfeMode::Mute,
        LfeMode::MixToFront => V2LfeMode::MixToFront,
    }
}

fn map_resample_quality(quality: ResampleQuality) -> V2ResampleQuality {
    match quality {
        ResampleQuality::Fast => V2ResampleQuality::Fast,
        ResampleQuality::Balanced => V2ResampleQuality::Balanced,
        ResampleQuality::High => V2ResampleQuality::High,
        ResampleQuality::Ultra => V2ResampleQuality::Ultra,
    }
}
