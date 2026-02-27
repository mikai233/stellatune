use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use crate::frb_generated::StreamSink;
use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use stellatune_runtime as global_runtime;
use tracing::{debug, warn};

use crate::api::library::shared_library_if_initialized;

mod plugin_ui_gateway;
pub(crate) mod types;
use stellatune_audio::config::engine::{
    Event as V2Event, LfeMode as V2LfeMode, PlayerState as V2PlayerState,
    ResampleQuality as V2ResampleQuality,
};
use stellatune_audio::engine::EngineHandle as AudioEngineHandle;
use stellatune_audio::pipeline::assembly::{BuiltinTransformSlot, PipelineMutation};
use stellatune_audio_plugin_adapters::pipeline::{
    PluginPipelineOrchestrator, PluginTransformSegment, PluginTransformStageSpec,
};
use stellatune_backend_api::lyrics_service::LyricsService;
use stellatune_backend_api::player::{
    plugins_install_from_file as backend_plugins_install_from_file,
    plugins_list_installed_json as backend_plugins_list_installed_json,
    plugins_uninstall_by_id as backend_plugins_uninstall_by_id,
};
use stellatune_backend_api::runtime::{
    OutputBackend as RuntimeOutputBackend,
    decoder_supported_extensions_hybrid as runtime_decoder_supported_extensions,
    list_local_transcode_encoders, open_local_transcode_decoder, open_local_transcode_encoder,
    probe_track_decode_info_hybrid, runtime_clear_output_sink_route, runtime_list_output_devices,
    runtime_set_output_device, runtime_set_output_options, runtime_set_output_sink_route,
    shared_plugin_runtime, shared_runtime_engine,
};
use stellatune_backend_api::{LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate};
use stellatune_plugins::runtime::model::RuntimePcmF32Chunk;
use types::{
    AudioBackend, AudioDevice, DspChainItem, DspTypeDescriptor, EncoderTypeDescriptor, Event,
    LfeMode, LyricsProviderTypeDescriptor, OutputSinkRoute, OutputSinkTypeDescriptor, PlayerState,
    PluginDescriptor, ResampleQuality, SourceCatalogTypeDescriptor, TrackDecodeInfo, TrackRef,
    TranscodeProgressEvent,
};

struct PlayerContext {
    engine: Arc<AudioEngineHandle>,
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

fn engine() -> Arc<AudioEngineHandle> {
    Arc::clone(&shared_player_context().engine)
}

fn lyrics() -> Arc<LyricsService> {
    Arc::clone(&shared_player_context().lyrics)
}

fn shared_transcode_cancel_flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static FLAGS: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    FLAGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_transcode_cancel_flag(task_id: &str) -> Result<Arc<AtomicBool>> {
    let mut guard = shared_transcode_cancel_flags()
        .lock()
        .map_err(|_| anyhow!("transcode cancel map is poisoned"))?;
    if guard.contains_key(task_id) {
        return Err(anyhow!("transcode task_id `{task_id}` is already running"));
    }
    let flag = Arc::new(AtomicBool::new(false));
    guard.insert(task_id.to_string(), Arc::clone(&flag));
    Ok(flag)
}

fn clear_transcode_cancel_flag(task_id: &str) {
    if let Ok(mut guard) = shared_transcode_cancel_flags().lock() {
        guard.remove(task_id);
    }
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
                            error = %error,
                            "failed to apply pending preload seek on track switch"
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
    shared_plugin_runtime()
        .active_plugins()
        .await
        .into_iter()
        .map(|plugin| PluginDescriptor {
            id: plugin.id,
            name: plugin.name,
        })
        .collect()
}

pub async fn dsp_list_types() -> Vec<DspTypeDescriptor> {
    let service = shared_plugin_runtime();
    let plugins = service.active_plugins().await;
    let mut out = Vec::new();
    for plugin in plugins {
        let mut capabilities = service.list_dsp_capabilities(&plugin.id);
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            out.push(DspTypeDescriptor {
                plugin_id: plugin.id.clone(),
                plugin_name: plugin.name.clone(),
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
    let plugins = service.active_plugins().await;
    let mut out = Vec::new();
    for plugin in plugins {
        let mut capabilities = service.list_source_capabilities(&plugin.id);
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            out.push(SourceCatalogTypeDescriptor {
                plugin_id: plugin.id.clone(),
                plugin_name: plugin.name.clone(),
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
    let plugins = service.active_plugins().await;
    let mut out = Vec::new();
    for plugin in plugins {
        let mut capabilities = service.list_lyrics_capabilities(&plugin.id);
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            out.push(LyricsProviderTypeDescriptor {
                plugin_id: plugin.id.clone(),
                plugin_name: plugin.name.clone(),
                type_id: capability.type_id,
                display_name: capability.display_name,
            });
        }
    }
    out
}

pub async fn output_sink_list_types() -> Vec<OutputSinkTypeDescriptor> {
    let service = shared_plugin_runtime();
    let plugins = service.active_plugins().await;
    let mut out = Vec::new();
    for plugin in plugins {
        let mut capabilities = service.list_output_sink_capabilities(&plugin.id);
        capabilities.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for capability in capabilities {
            out.push(OutputSinkTypeDescriptor {
                plugin_id: plugin.id.clone(),
                plugin_name: plugin.name.clone(),
                type_id: capability.type_id,
                display_name: capability.display_name,
                config_schema_json: capability.config_schema_json,
                default_config_json: capability.default_config_json,
            });
        }
    }
    out
}

pub async fn encoder_list_types() -> Vec<EncoderTypeDescriptor> {
    list_local_transcode_encoders()
        .into_iter()
        .map(|item| EncoderTypeDescriptor {
            plugin_id: item.plugin_id,
            plugin_name: item.plugin_name,
            type_id: item.type_id,
            display_name: item.display_name,
            config_schema_json: item.config_schema_json,
            default_config_json: item.default_config_json,
        })
        .collect()
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
    let mut source = shared_plugin_runtime()
        .create_source_plugin(&plugin_id, &type_id)
        .map_err(|e| anyhow!("create source plugin failed: {e}"))?;
    source
        .apply_config_update_json(
            &serde_json::to_string(&config)
                .map_err(|e| anyhow!("serialize source config_json: {e}"))?,
        )
        .map_err(|e| anyhow!("source apply_config_update_json failed: {e}"))?;
    let payload = source
        .list_items_json(
            &serde_json::to_string(&request)
                .map_err(|e| anyhow!("serialize source request_json: {e}"))?,
        )
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
    let keyword = extract_lyrics_search_keyword(&query)
        .ok_or_else(|| anyhow!("lyrics query_json missing keyword"))?;
    let mut lyrics = shared_plugin_runtime()
        .create_lyrics_plugin(&plugin_id, &type_id)
        .map_err(|e| anyhow!("create lyrics plugin failed: {e}"))?;
    let payload = lyrics
        .search_json(&keyword)
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
    let lyric_id =
        extract_lyrics_fetch_id(&track).ok_or_else(|| anyhow!("lyrics track_json missing id"))?;
    let mut lyrics = shared_plugin_runtime()
        .create_lyrics_plugin(&plugin_id, &type_id)
        .map_err(|e| anyhow!("create lyrics plugin failed: {e}"))?;
    let payload = lyrics
        .fetch_text(&lyric_id)
        .map_err(|e| anyhow!("lyrics fetch failed: {e}"))?;
    normalize_json_payload("lyrics fetch response", serde_json::Value::String(payload))
}

pub async fn output_sink_list_targets_json(
    plugin_id: String,
    type_id: String,
    config_json: String,
) -> Result<String> {
    let config = serde_json::from_str::<serde_json::Value>(&config_json)
        .map_err(|e| anyhow!("invalid output sink config_json: {e}"))?;
    let mut output_sink = shared_plugin_runtime()
        .create_output_sink_plugin(&plugin_id, &type_id)
        .map_err(|e| anyhow!("create output sink plugin failed: {e}"))?;
    output_sink
        .apply_config_update_json(
            &serde_json::to_string(&config)
                .map_err(|e| anyhow!("serialize output sink config_json: {e}"))?,
        )
        .map_err(|e| anyhow!("output sink apply_config_update_json failed: {e}"))?;
    let payload = output_sink
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
                error = %error,
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
            warn!(error = %error, "current_track_info snapshot failed");
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

    let info = match probe_track_decode_info_hybrid(track_token.as_str()) {
        Ok(probed) => Some(TrackDecodeInfo {
            sample_rate: probed.sample_rate,
            channels: probed.channels,
            duration_ms: probed.duration_ms,
            metadata_json: probed.metadata_json,
            decoder_plugin_id: probed.decoder_plugin_id,
            decoder_type_id: probed.decoder_type_id,
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
    let installed_plugin_id = tokio::task::spawn_blocking(move || {
        backend_plugins_install_from_file(plugins_dir, artifact_path)
    })
    .await
    .map_err(|e| anyhow!("JoinError: {e}"))??;
    reconcile_plugin_runtime_state_after_package_change("install").await?;
    Ok(installed_plugin_id)
}

pub async fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    tokio::task::spawn_blocking(move || backend_plugins_list_installed_json(plugins_dir))
        .await
        .map_err(|e| anyhow!("JoinError: {e}"))?
}

pub async fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    tokio::task::spawn_blocking(move || backend_plugins_uninstall_by_id(plugins_dir, plugin_id))
        .await
        .map_err(|e| anyhow!("JoinError: {e}"))??;
    reconcile_plugin_runtime_state_after_package_change("uninstall").await
}

pub async fn plugin_ui_gateway_start(plugins_dir: String, port: Option<u16>) -> Result<String> {
    plugin_ui_gateway::start(plugins_dir, port).await
}

pub async fn plugin_ui_gateway_stop() -> Result<()> {
    plugin_ui_gateway::stop().await
}

pub async fn plugin_ui_gateway_base_url() -> Option<String> {
    plugin_ui_gateway::base_url().await
}

pub async fn plugin_ui_gateway_session_token() -> Option<String> {
    plugin_ui_gateway::session_token().await
}

pub async fn plugin_ui_gateway_plugin_ui_url(plugin_id: String) -> Option<String> {
    plugin_ui_gateway::plugin_ui_url(plugin_id).await
}

async fn reconcile_plugin_runtime_state_after_package_change(operation: &str) -> Result<()> {
    let Some(library) = shared_library_if_initialized() else {
        return Ok(());
    };
    library
        .plugin_apply_state()
        .await
        .with_context(|| format!("failed to apply plugin runtime state after {operation}"))
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

#[derive(Debug, Clone)]
pub struct TranscodeTrackLocalRequest {
    pub task_id: String,
    pub source_path: String,
    pub output_path: String,
    pub encoder_plugin_id: String,
    pub encoder_type_id: String,
    pub encoder_config_json: String,
    pub encoder_options_json: Option<String>,
}

impl TranscodeTrackLocalRequest {
    pub(crate) fn new(
        task_id: String,
        source_path: String,
        output_path: String,
        encoder_plugin_id: String,
        encoder_type_id: String,
        encoder_config_json: String,
        encoder_options_json: Option<String>,
    ) -> Result<Self> {
        let task_id = task_id.trim().to_string();
        let source_path = source_path.trim().to_string();
        let output_path = output_path.trim().to_string();
        let encoder_plugin_id = encoder_plugin_id.trim().to_string();
        let encoder_type_id = encoder_type_id.trim().to_string();
        if task_id.is_empty() {
            return Err(anyhow!("task_id is empty"));
        }
        if source_path.is_empty() {
            return Err(anyhow!("source_path is empty"));
        }
        if output_path.is_empty() {
            return Err(anyhow!("output_path is empty"));
        }
        if encoder_plugin_id.is_empty() {
            return Err(anyhow!("encoder_plugin_id is empty"));
        }
        if encoder_type_id.is_empty() {
            return Err(anyhow!("encoder_type_id is empty"));
        }
        Ok(Self {
            task_id,
            source_path,
            output_path,
            encoder_plugin_id,
            encoder_type_id,
            encoder_config_json,
            encoder_options_json,
        })
    }
}

struct TranscodeTaskContext {
    request: TranscodeTrackLocalRequest,
    cancel_flag: Arc<AtomicBool>,
    sink: StreamSink<TranscodeProgressEvent>,
}

pub fn transcode_track_local(
    request: TranscodeTrackLocalRequest,
    sink: StreamSink<TranscodeProgressEvent>,
) -> Result<()> {
    let request = TranscodeTrackLocalRequest::new(
        request.task_id,
        request.source_path,
        request.output_path,
        request.encoder_plugin_id,
        request.encoder_type_id,
        request.encoder_config_json,
        request.encoder_options_json,
    )?;
    let cancel_flag = register_transcode_cancel_flag(request.task_id.as_str())?;

    global_runtime::spawn(async move {
        let task_id_for_worker = request.task_id.clone();
        let worker = tokio::task::spawn_blocking(move || {
            run_transcode_track_local_blocking(TranscodeTaskContext {
                request,
                cancel_flag,
                sink,
            })
        });
        match worker.await {
            Ok(()) => {},
            Err(error) => {
                warn!(error = %error, "transcode worker join failed");
            },
        }
        clear_transcode_cancel_flag(task_id_for_worker.as_str());
    });

    Ok(())
}

pub fn transcode_cancel(task_id: String) -> Result<()> {
    let task_id = task_id.trim();
    if task_id.is_empty() {
        return Err(anyhow!("task_id is empty"));
    }
    let guard = shared_transcode_cancel_flags()
        .lock()
        .map_err(|_| anyhow!("transcode cancel map is poisoned"))?;
    if let Some(flag) = guard.get(task_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

fn run_transcode_track_local_blocking(context: TranscodeTaskContext) {
    const MAX_DECODE_FRAMES: u32 = 8192;
    let TranscodeTaskContext {
        request:
            TranscodeTrackLocalRequest {
                task_id,
                source_path,
                output_path,
                encoder_plugin_id,
                encoder_type_id,
                encoder_config_json,
                encoder_options_json,
            },
        cancel_flag,
        sink,
    } = context;

    let started_at = Instant::now();
    let mut processed_frames = 0u64;
    let mut written_bytes = 0u64;
    let mut total_frames = None::<u64>;
    let mut sample_rate = None::<u32>;
    let mut channels = None::<u16>;

    let source_path_for_failed = source_path.clone();
    let output_path_for_failed = output_path.clone();
    let mut output_file_existed_before = true;

    let result = (|| -> Result<()> {
        ensure_transcode_not_canceled(cancel_flag.as_ref())?;
        let source = Path::new(source_path.as_str());
        if !source.exists() {
            return Err(anyhow!("source file does not exist: {}", source.display()));
        }
        if !source.is_file() {
            return Err(anyhow!("source path is not a file: {}", source.display()));
        }

        let output = Path::new(output_path.as_str());
        if let Some(parent) = output.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory: {}", parent.display())
            })?;
        }
        output_file_existed_before = output.exists();

        let mut decoder = open_local_transcode_decoder(source_path.as_str())
            .map_err(|error| anyhow!("failed to open local transcode decoder: {error}"))?;
        let decoder_info = decoder.info();

        sample_rate = Some(decoder_info.sample_rate);
        channels = Some(decoder_info.channels);
        total_frames = estimate_total_frames(decoder_info.duration_ms, decoder_info.sample_rate);

        let encoder_config_json = normalize_optional_json(encoder_config_json)?.unwrap_or_default();
        let encoder_options_json = normalize_optional_json_opt(encoder_options_json)?;
        let mut encoder = open_local_transcode_encoder(
            output_path.as_str(),
            encoder_plugin_id.as_str(),
            encoder_type_id.as_str(),
            decoder_info.sample_rate,
            decoder_info.channels,
            decoder_info.metadata.clone(),
            encoder_config_json.as_str(),
            encoder_options_json.as_deref(),
        )
        .map_err(|error| anyhow!("failed to open local transcode encoder: {error}"))?;

        emit_transcode_event(
            &sink,
            TranscodeProgressEvent {
                phase: "started".to_string(),
                message: None,
                source_path: Some(source_path.clone()),
                output_path: Some(output_path.clone()),
                processed_frames,
                total_frames,
                written_bytes,
                sample_rate,
                channels,
                elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
            },
        )?;

        loop {
            ensure_transcode_not_canceled(cancel_flag.as_ref())?;
            let pcm = decoder
                .read_pcm_f32(MAX_DECODE_FRAMES)
                .map_err(|error| anyhow!("decoder read failed: {error}"))?;
            let consumed = encoder
                .write_pcm_f32(RuntimePcmF32Chunk {
                    interleaved_f32le: pcm.interleaved_f32le,
                    frames: pcm.frames,
                    eof: pcm.eof,
                })
                .map_err(|error| anyhow!("encoder write failed: {error}"))?;
            if consumed != pcm.frames {
                return Err(anyhow!(
                    "encoder partial consume is not supported yet (consumed={} frames={})",
                    consumed,
                    pcm.frames
                ));
            }
            processed_frames = processed_frames.saturating_add(consumed as u64);
            written_bytes = encoder.written_bytes();
            emit_transcode_event(
                &sink,
                TranscodeProgressEvent {
                    phase: "progress".to_string(),
                    message: None,
                    source_path: Some(source_path.clone()),
                    output_path: Some(output_path.clone()),
                    processed_frames,
                    total_frames,
                    written_bytes,
                    sample_rate,
                    channels,
                    elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
                },
            )?;

            if pcm.eof {
                break;
            }
        }

        encoder
            .finish()
            .map_err(|error| anyhow!("encoder finish failed: {error}"))?;
        written_bytes = encoder.written_bytes();

        let _ = sink.add(TranscodeProgressEvent {
            phase: "completed".to_string(),
            message: None,
            source_path: Some(source_path.clone()),
            output_path: Some(output_path.clone()),
            processed_frames,
            total_frames,
            written_bytes,
            sample_rate,
            channels,
            elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
        });

        // Keep best-effort explicit close; handles also clean up on drop.
        let _ = encoder.close();
        let _ = decoder.close();
        Ok(())
    })();

    if let Err(error) = result {
        let canceled = cancel_flag.load(Ordering::Relaxed);
        warn!(
            task_id = %task_id,
            source_path = %source_path_for_failed,
            output_path = %output_path_for_failed,
            error = %error,
            "transcode_track_local failed"
        );
        if !output_file_existed_before {
            let _ = fs::remove_file(output_path_for_failed.as_str());
        }
        let _ = sink.add(TranscodeProgressEvent {
            phase: if canceled { "canceled" } else { "failed" }.to_string(),
            message: if canceled {
                None
            } else {
                Some(error.to_string())
            },
            source_path: Some(source_path_for_failed),
            output_path: Some(output_path_for_failed),
            processed_frames,
            total_frames,
            written_bytes,
            sample_rate,
            channels,
            elapsed_ms: Some(started_at.elapsed().as_millis() as u64),
        });
    }
}

fn normalize_optional_json(raw: String) -> Result<Option<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| anyhow!("invalid json payload: {e}"))?;
    let normalized = serde_json::to_string(&value).map_err(|e| anyhow!("serialize json: {e}"))?;
    Ok(Some(normalized))
}

fn normalize_optional_json_opt(raw: Option<String>) -> Result<Option<String>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    normalize_optional_json(raw)
}

fn estimate_total_frames(duration_ms: Option<u64>, sample_rate: u32) -> Option<u64> {
    let duration_ms = duration_ms?;
    let sample_rate = sample_rate.max(1);
    let frames = (duration_ms as u128)
        .saturating_mul(sample_rate as u128)
        .saturating_div(1000);
    Some(frames.min(u64::MAX as u128) as u64)
}

fn emit_transcode_event(
    sink: &StreamSink<TranscodeProgressEvent>,
    event: TranscodeProgressEvent,
) -> Result<()> {
    sink.add(event)
        .map_err(|_| anyhow!("transcode progress stream closed"))
}

fn ensure_transcode_not_canceled(cancel_flag: &AtomicBool) -> Result<()> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err(anyhow!("transcode canceled"));
    }
    Ok(())
}

pub async fn decoder_supported_extensions() -> Vec<String> {
    runtime_decoder_supported_extensions()
}

fn normalize_json_payload(label: &str, payload: serde_json::Value) -> Result<String> {
    serde_json::to_string(&payload).map_err(|e| anyhow!("serialize {label}: {e}"))
}

fn normalize_json_string_payload(label: &str, payload: String) -> Result<String> {
    let value = serde_json::from_str::<serde_json::Value>(&payload)
        .map_err(|e| anyhow!("deserialize {label}: {e}"))?;
    normalize_json_payload(label, value)
}

fn extract_lyrics_search_keyword(query: &serde_json::Value) -> Option<String> {
    match query {
        serde_json::Value::String(s) => {
            let s = s.trim();
            (!s.is_empty()).then_some(s.to_string())
        },
        serde_json::Value::Object(map) => ["keyword", "keywords", "query", "q", "title"]
            .into_iter()
            .find_map(|key| map.get(key).and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        _ => None,
    }
}

fn extract_lyrics_fetch_id(track: &serde_json::Value) -> Option<String> {
    match track {
        serde_json::Value::String(s) => {
            let s = s.trim();
            (!s.is_empty()).then_some(s.to_string())
        },
        serde_json::Value::Object(map) => ["id", "lyric_id", "lyricId", "track_id", "trackId"]
            .into_iter()
            .find_map(|key| map.get(key).and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        _ => None,
    }
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
        V2Event::AudioStart => vec![Event::AudioStart],
        V2Event::AudioEnd => vec![Event::AudioEnd],
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
