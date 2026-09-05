use std::sync::{Arc, Mutex, OnceLock};

use crate::frb_generated::StreamSink;
use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use tracing::{debug, warn};

use crate::api::library::{shared_library_if_initialized, shared_player_service};

mod host_api;
pub mod queue;
pub mod transcode;
pub mod types;
use stellatune_audio::config::engine::{
    LfeMode as V2LfeMode, ResampleQuality as V2ResampleQuality,
};
use stellatune_audio::playback::control::PlaybackController;
use stellatune_audio::playback::event::{PlaybackEvent as V2Event, PlaybackState as V2PlayerState};
use stellatune_audio_core::playback::{MediaTime, PlaybackItemId};
use stellatune_backend_api::lyrics_service::LyricsService;
use stellatune_backend_api::player::{
    plugins_install_from_file as backend_plugins_install_from_file,
    plugins_list_installed_json as backend_plugins_list_installed_json,
    plugins_uninstall_by_id as backend_plugins_uninstall_by_id,
};
use stellatune_backend_api::player_service::identity::TrackId;
use stellatune_backend_api::runtime::{
    OutputBackend as RuntimeOutputBackend,
    decoder_supported_extensions as runtime_decoder_supported_extensions,
    list_local_transcode_encoders, probe_local_track, runtime_clear_output_sink_route,
    runtime_list_output_devices, runtime_set_output_device, runtime_set_output_options,
    runtime_set_output_sink_route, set_runtime_builtin_transform_options,
    shared_playback_controller, shared_typescript_runtime,
};
use stellatune_backend_api::{LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate};
use types::{
    AudioBackend, AudioDevice, DspChainItem, DspTypeDescriptor, EncoderTypeDescriptor, Event,
    LfeMode, LyricsProviderTypeDescriptor, OutputSinkRoute, OutputSinkTypeDescriptor,
    PlaybackSnapshot, PlayerState, PluginDescriptor, ResampleQuality, SourceCatalogTypeDescriptor,
    TrackDecodeInfo,
};

struct PlayerContext {
    engine: PlaybackController,
    lyrics: Arc<LyricsService>,
    track_info_cache: Arc<Mutex<Option<CachedTrackDecodeInfo>>>,
}

#[derive(Debug, Clone)]
struct CachedTrackDecodeInfo {
    item_id: PlaybackItemId,
    info: Option<TrackDecodeInfo>,
}

#[derive(Debug, Clone)]
#[flutter_rust_bridge::frb(ignore)]
struct FfiEventMapperState {
    current_track_id: Option<TrackId>,
    current_item_id: Option<PlaybackItemId>,
    position_session_id: u64,
    recovering: bool,
    last_player_state: PlayerState,
}

impl Default for FfiEventMapperState {
    fn default() -> Self {
        Self {
            current_track_id: None,
            current_item_id: None,
            position_session_id: 0,
            recovering: false,
            last_player_state: PlayerState::Stopped,
        }
    }
}

fn shared_player_context() -> &'static PlayerContext {
    static CONTEXT: OnceLock<PlayerContext> = OnceLock::new();
    CONTEXT.get_or_init(|| PlayerContext {
        engine: shared_playback_controller(),
        lyrics: LyricsService::new(),
        track_info_cache: Arc::new(Mutex::new(None)),
    })
}

fn engine() -> PlaybackController {
    shared_player_context().engine.clone()
}

fn lyrics() -> Arc<LyricsService> {
    Arc::clone(&shared_player_context().lyrics)
}

/// Registers a local queue in one catalog transaction, preserving input order.
pub async fn ensure_local_tracks(library_track_ids: Vec<i64>) -> Result<Vec<u64>> {
    Ok(shared_player_service()?
        .ensure_local_tracks(&library_track_ids)
        .await?
        .into_iter()
        .map(|id| id.get())
        .collect())
}

pub async fn ensure_local_track(library_track_id: i64) -> Result<u64> {
    Ok(shared_player_service()?
        .ensure_local_track(library_track_id)
        .await?
        .get())
}

pub async fn ensure_provider_track(
    provider_id: String,
    provider_key: String,
    plugin_id: String,
    type_id: String,
) -> Result<u64> {
    Ok(
        stellatune_backend_api::player_service::plugin_tracks::ensure_provider_track(
            shared_player_service()?.as_ref(),
            shared_typescript_runtime(),
            &plugin_id,
            &type_id,
            &provider_id,
            &provider_key,
        )
        .await?
        .get(),
    )
}

pub async fn play() -> Result<()> {
    shared_player_service()?
        .play()
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn pause() -> Result<()> {
    engine().pause().await.map_err(anyhow::Error::msg)
}

pub async fn seek_ms(position_ms: u64) -> Result<()> {
    engine()
        .seek(MediaTime::from_millis(position_ms))
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_volume(volume: f32, seq: u64, ramp_ms: u32) -> Result<()> {
    let _ = seq;
    engine()
        .set_output_gain(volume, MediaTime::from_millis(u64::from(ramp_ms)))
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_lfe_mode(mode: LfeMode) -> Result<()> {
    let _ = map_lfe_mode(mode);
    Ok(())
}

pub async fn stop() -> Result<()> {
    let result = shared_player_service()?
        .stop()
        .await
        .map_err(anyhow::Error::msg);
    if result.is_ok() {
        clear_cached_track_info();
    }
    result
}

pub async fn playback_snapshot() -> Result<PlaybackSnapshot> {
    let snapshot = engine().snapshot().await.map_err(anyhow::Error::msg)?;
    let (track_id, local_library_track_id) = if let Some(item_id) = snapshot.current_item_id {
        let service = shared_player_service()?;
        (
            Some(service.track_id_for_item(item_id).await?.get()),
            service.local_library_track_id_for_item(item_id).await?,
        )
    } else {
        (None, None)
    };
    Ok(PlaybackSnapshot {
        state: map_player_state(snapshot.state),
        track_id,
        item_id: snapshot.current_item_id.map(PlaybackItemId::get),
        local_library_track_id,
        position_ms: snapshot.consumed_position.as_millis().min(i64::MAX as u64) as i64,
    })
}

pub fn events(sink: StreamSink<Event>) -> Result<()> {
    let mut rx = engine().subscribe_events();
    crate::background_runtime::spawn(async move {
        let mut state = FfiEventMapperState::default();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let event_item_id = playback_event_item_id(&event);
                    let event_track_id = if let Some(item_id) = event_item_id
                        && let Ok(service) = shared_player_service()
                    {
                        service.track_id_for_item(item_id).await.ok()
                    } else {
                        None
                    };

                    let mapped = map_v2_event_to_ffi(event, event_track_id, &mut state);
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
    crate::background_runtime::spawn(async move {
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
    let mut plugins = stellatune_backend_api::runtime::shared_typescript_runtime()
        .registered_plugins()
        .await
        .into_iter()
        .map(|plugin| PluginDescriptor {
            id: plugin.manifest.id,
            name: plugin.manifest.name,
        })
        .collect::<Vec<_>>();
    plugins.sort_by(|left, right| left.id.cmp(&right.id));
    plugins
}

pub async fn dsp_list_types() -> Vec<DspTypeDescriptor> {
    Vec::new()
}

pub async fn source_list_types() -> Vec<SourceCatalogTypeDescriptor> {
    use stellatune_plugins::typescript::manifest::TypeScriptCapabilityKind;

    let plugins = stellatune_backend_api::runtime::shared_typescript_runtime()
        .registered_plugins()
        .await;
    let mut out = Vec::new();
    for plugin in plugins {
        for capability in plugin.manifest.capabilities.iter().filter(|capability| {
            matches!(capability.kind, TypeScriptCapabilityKind::NetworkControl)
        }) {
            out.push(SourceCatalogTypeDescriptor {
                plugin_id: plugin.manifest.id.clone(),
                plugin_name: plugin.manifest.name.clone(),
                type_id: capability.id.clone(),
                display_name: capability.display_name.clone(),
            });
        }
    }
    out.sort_by(|left, right| {
        left.plugin_id
            .cmp(&right.plugin_id)
            .then_with(|| left.type_id.cmp(&right.type_id))
    });
    out
}

pub async fn lyrics_provider_list_types() -> Vec<LyricsProviderTypeDescriptor> {
    use stellatune_plugins::typescript::manifest::TypeScriptCapabilityKind;
    let plugins = shared_typescript_runtime().registered_plugins().await;
    let mut out = Vec::new();
    for plugin in plugins {
        for capability in plugin
            .manifest
            .capabilities
            .iter()
            .filter(|capability| capability.kind == TypeScriptCapabilityKind::LyricsProvider)
        {
            out.push(LyricsProviderTypeDescriptor {
                plugin_id: plugin.manifest.id.clone(),
                plugin_name: plugin.manifest.name.clone(),
                type_id: capability.id.clone(),
                display_name: capability.display_name.clone(),
            });
        }
    }
    out
}

pub async fn output_sink_list_types() -> Vec<OutputSinkTypeDescriptor> {
    Vec::new()
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
    request_json: String,
) -> Result<String> {
    use stellatune_plugins::typescript::manifest::TypeScriptCapabilityKind;
    let request: Value = serde_json::from_str(&request_json)?;
    let runtime = shared_typescript_runtime();
    let registrations = runtime.registered_plugins().await;
    let capability = registrations
        .iter()
        .find(|p| p.manifest.id == plugin_id)
        .and_then(|p| p.manifest.capabilities.iter().find(|c| c.id == type_id))
        .ok_or_else(|| anyhow!("plugin capability is not registered: {plugin_id}::{type_id}"))?;
    if capability.kind != TypeScriptCapabilityKind::NetworkControl {
        return Err(anyhow!("catalog capability must be network-control"));
    }
    let result = runtime
        .invoke(&plugin_id, &type_id, None, "list-items", request, None)
        .await?;
    normalize_json_payload("TypeScript catalog response", result.value)
}

pub async fn lyrics_provider_search_json(
    plugin_id: String,
    type_id: String,
    query_json: String,
) -> Result<String> {
    let query = serde_json::from_str::<serde_json::Value>(&query_json)
        .map_err(|e| anyhow!("invalid lyrics query_json: {e}"))?;
    let result = shared_typescript_runtime()
        .invoke(&plugin_id, &type_id, None, "search", query, None)
        .await
        .map_err(|error| anyhow!("lyrics search failed: {error}"))?;
    normalize_json_payload("lyrics search response", result.value)
}

pub async fn lyrics_provider_fetch_json(
    plugin_id: String,
    type_id: String,
    track_json: String,
) -> Result<String> {
    let track = serde_json::from_str::<serde_json::Value>(&track_json)
        .map_err(|e| anyhow!("invalid lyrics track_json: {e}"))?;
    let result = shared_typescript_runtime()
        .invoke(&plugin_id, &type_id, None, "fetch", track, None)
        .await
        .map_err(|error| anyhow!("lyrics fetch failed: {error}"))?;
    normalize_json_payload("lyrics fetch response", result.value)
}

pub async fn output_sink_list_targets_json(
    plugin_id: String,
    type_id: String,
    config_json: String,
) -> Result<String> {
    let _ = (plugin_id, type_id, config_json);
    Ok("[]".to_string())
}

pub async fn dsp_set_chain(chain: Vec<DspChainItem>) {
    if !chain.is_empty() {
        warn!("TypeScript DSP stages are unsupported; ignoring requested DSP chain");
    }
    if let Err(error) = engine().rebuild_output().await {
        warn!(error = %error, "failed to rebuild typed DSP chain");
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
    let Some(item_id) = snapshot.current_item_id else {
        clear_cached_track_info();
        return None;
    };

    if let Ok(cache_guard) = shared_player_context().track_info_cache.lock()
        && let Some(entry) = cache_guard.as_ref()
        && entry.item_id == item_id
    {
        return entry.info.clone();
    }

    let path = match shared_player_service() {
        Ok(service) => match service.local_path_for_item(item_id).await {
            Ok(Some(path)) => path,
            Ok(None) => return None,
            Err(error) => {
                warn!(%error, "current track path resolution failed");
                return None;
            },
        },
        Err(error) => {
            warn!(%error, "player service unavailable for current track probe");
            return None;
        },
    };
    let info = match probe_local_track(&path).await {
        Ok(probed) => Some(TrackDecodeInfo {
            sample_rate: probed.sample_rate,
            channels: probed.channels,
            duration_ms: probed.duration_ms,
            metadata_json: probed.metadata_json,
            decoder_plugin_id: probed.decoder_plugin_id,
            decoder_type_id: probed.decoder_type_id,
        }),
        Err(error) => {
            warn!(
                item_id = item_id.get(),
                error, "current_track_info probe failed"
            );
            None
        },
    };

    if let Ok(mut cache_guard) = shared_player_context().track_info_cache.lock() {
        *cache_guard = Some(CachedTrackDecodeInfo {
            item_id,
            info: info.clone(),
        });
    }
    info
}

pub async fn plugins_install_from_file(
    plugins_dir: String,
    artifact_path: String,
) -> Result<String> {
    let installed_plugin_id = backend_plugins_install_from_file(plugins_dir, artifact_path).await?;
    reconcile_plugin_runtime_state_after_package_change("install").await?;
    Ok(installed_plugin_id)
}

pub async fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    tokio::task::spawn_blocking(move || backend_plugins_list_installed_json(plugins_dir))
        .await
        .map_err(|e| anyhow!("JoinError: {e}"))?
}

pub async fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    backend_plugins_uninstall_by_id(plugins_dir, plugin_id).await?;
    reconcile_plugin_runtime_state_after_package_change("uninstall").await
}

pub async fn host_api_start(data_root: String) -> Result<String> {
    host_api::start(data_root).await
}

/// Restore only after host initialization, plugin registration and output settings.
pub async fn playback_restore_state() -> Result<()> {
    let service = shared_player_service()?;
    let restored = service.restore().await;
    service.start_state_writer();
    restored?;
    Ok(())
}

pub async fn host_api_stop() -> Result<()> {
    host_api::stop().await;
    Ok(())
}

pub async fn plugin_open_ui(plugin_id: String) -> Result<String> {
    Ok(shared_typescript_runtime().open_ui(&plugin_id).await?)
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
    let mapped_quality = map_resample_quality(resample_quality);
    set_runtime_builtin_transform_options(gapless_playback, seek_track_fade)
        .await
        .map_err(anyhow::Error::msg)?;
    runtime_set_output_options(match_track_sample_rate, mapped_quality)
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

pub async fn decoder_supported_extensions() -> Vec<String> {
    runtime_decoder_supported_extensions()
}

fn normalize_json_payload(label: &str, payload: serde_json::Value) -> Result<String> {
    serde_json::to_string(&payload).map_err(|e| anyhow!("serialize {label}: {e}"))
}

fn clear_cached_track_info() {
    if let Ok(mut cache_guard) = shared_player_context().track_info_cache.lock() {
        *cache_guard = None;
    }
}

fn next_position_session_id(current: &mut u64) -> u64 {
    *current = current.wrapping_add(1);
    if *current == 0 {
        *current = 1;
    }
    *current
}

fn playback_event_item_id(event: &V2Event) -> Option<PlaybackItemId> {
    match event {
        V2Event::TrackChanged { item_id }
        | V2Event::PlaybackEnded { item_id }
        | V2Event::Position { item_id, .. }
        | V2Event::Buffering { item_id, .. } => Some(*item_id),
        V2Event::Failed(failure) => failure.item_id,
        V2Event::StateChanged(_) => None,
    }
}

fn map_v2_event_to_ffi(
    event: V2Event,
    event_track_id: Option<TrackId>,
    state: &mut FfiEventMapperState,
) -> Vec<Event> {
    match event {
        V2Event::StateChanged(next_state) => {
            let mapped = map_player_state(next_state);
            state.last_player_state = mapped;
            state.recovering = false;
            vec![Event::StateChanged { state: mapped }]
        },
        V2Event::TrackChanged { item_id } => {
            let Some(track_id) = event_track_id else {
                return vec![Event::Error {
                    message: format!("missing TrackId for playback item {}", item_id.get()),
                }];
            };
            state.current_track_id = Some(track_id);
            state.current_item_id = Some(item_id);
            let _ = next_position_session_id(&mut state.position_session_id);
            vec![Event::TrackChanged {
                track_id: track_id.get(),
                item_id: item_id.get(),
            }]
        },
        V2Event::PlaybackEnded { item_id } => {
            let track_id = event_track_id.or(state.current_track_id);
            let Some(track_id) = track_id else {
                return vec![Event::Error {
                    message: format!("missing TrackId for ended playback item {}", item_id.get()),
                }];
            };
            vec![Event::PlaybackEnded {
                track_id: track_id.get(),
                item_id: item_id.get(),
            }]
        },
        V2Event::Position { item_id, position } => {
            let track_id = event_track_id.or(state.current_track_id);
            let Some(track_id) = track_id else {
                return Vec::new();
            };
            let mut out = Vec::with_capacity(2);
            if state.recovering {
                state.recovering = false;
                out.push(Event::StateChanged {
                    state: state.last_player_state,
                });
            }
            out.push(Event::Position {
                ms: position.as_millis().min(i64::MAX as u64) as i64,
                track_id: track_id.get(),
                item_id: item_id.get(),
                session_id: state.position_session_id,
            });
            out
        },
        V2Event::Buffering { active, .. } => {
            state.recovering = active;
            vec![Event::StateChanged {
                state: if active {
                    PlayerState::Buffering
                } else {
                    state.last_player_state
                },
            }]
        },
        V2Event::Failed(failure) => {
            state.recovering = false;
            vec![Event::Error {
                message: failure.message,
            }]
        },
    }
}

fn map_player_state(state: V2PlayerState) -> PlayerState {
    match state {
        V2PlayerState::Idle | V2PlayerState::Failed => PlayerState::Stopped,
        V2PlayerState::Preparing | V2PlayerState::Recovering | V2PlayerState::Buffering => {
            PlayerState::Buffering
        },
        V2PlayerState::Ready | V2PlayerState::Paused => PlayerState::Paused,
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
