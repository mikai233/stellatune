use std::sync::{Arc, OnceLock};
use std::thread;

use crate::frb_generated::StreamSink;
use anyhow::{Result, anyhow};
use tracing::debug;

use stellatune_audio::EngineHandle;
use stellatune_backend_api::lyrics_service::LyricsService;
use stellatune_backend_api::player::{
    plugins_install_from_file as backend_plugins_install_from_file,
    plugins_list_installed_json as backend_plugins_list_installed_json,
    plugins_uninstall_by_id as backend_plugins_uninstall_by_id,
};
use stellatune_backend_api::runtime::shared_runtime_engine;
use stellatune_core::{
    AudioBackend, AudioDevice, DspChainItem, DspTypeDescriptor, Event, LyricsDoc, LyricsEvent,
    LyricsProviderTypeDescriptor, LyricsQuery, LyricsSearchCandidate, OutputSinkRoute,
    OutputSinkTypeDescriptor, PluginDescriptor, PluginRuntimeEvent, SourceCatalogTypeDescriptor,
    TrackDecodeInfo, TrackPlayability, TrackRef,
};

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
        .switch_track_ref_async(track, lazy)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn play() -> Result<()> {
    engine().play_async().await.map_err(anyhow::Error::msg)
}

pub async fn pause() -> Result<()> {
    engine().pause_async().await.map_err(anyhow::Error::msg)
}

pub async fn seek_ms(position_ms: u64) -> Result<()> {
    engine()
        .seek_ms_async(position_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_volume(volume: f32) -> Result<()> {
    engine()
        .set_volume_async(volume)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn stop() -> Result<()> {
    engine().stop_async().await.map_err(anyhow::Error::msg)
}

pub fn events(sink: StreamSink<Event>) -> Result<()> {
    let rx = engine().subscribe_events();

    thread::Builder::new()
        .name("stellatune-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    debug!("events stream sink closed");
                    break;
                }
            }
        })
        .map_err(|e| anyhow!("failed to spawn stellatune-events thread: {e}"))?;

    Ok(())
}

pub fn plugin_runtime_events_global(sink: StreamSink<PluginRuntimeEvent>) -> Result<()> {
    let rx = stellatune_backend_api::runtime::subscribe_plugin_runtime_events_global();

    thread::Builder::new()
        .name("stellatune-plugin-runtime-events-global".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    debug!("plugin_runtime_events_global stream sink closed");
                    break;
                }
            }
        })
        .map_err(|e| {
            anyhow!("failed to spawn stellatune-plugin-runtime-events-global thread: {e}")
        })?;

    Ok(())
}

pub fn lyrics_prepare(query: LyricsQuery) -> Result<()> {
    lyrics().prepare(query)
}

pub fn lyrics_prefetch(query: LyricsQuery) -> Result<()> {
    lyrics().prefetch(query)
}

pub async fn lyrics_search_candidates(query: LyricsQuery) -> Result<Vec<LyricsSearchCandidate>> {
    lyrics().search_candidates(query).await
}

pub fn lyrics_apply_candidate(track_key: String, doc: LyricsDoc) -> Result<()> {
    lyrics().apply_candidate(track_key, doc)
}

pub async fn lyrics_set_cache_db_path(db_path: String) -> Result<()> {
    lyrics().set_cache_db_path(db_path).await
}

pub async fn lyrics_clear_cache() -> Result<()> {
    lyrics().clear_cache().await
}

pub fn lyrics_refresh_current() -> Result<()> {
    lyrics().refresh_current()
}

pub fn lyrics_set_position_ms(position_ms: u64) {
    lyrics().set_position_ms(position_ms);
}

pub fn lyrics_events(sink: StreamSink<LyricsEvent>) -> Result<()> {
    let rx = lyrics().subscribe_events();

    thread::Builder::new()
        .name("stellatune-lyrics-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    debug!("lyrics_events stream sink closed");
                    break;
                }
            }
        })
        .map_err(|e| anyhow!("failed to spawn stellatune-lyrics-events thread: {e}"))?;

    Ok(())
}

pub fn plugins_list() -> Vec<PluginDescriptor> {
    engine().list_plugins()
}

pub fn plugin_publish_event_json(plugin_id: Option<String>, event_json: String) -> Result<()> {
    engine()
        .plugin_publish_event_json(plugin_id, event_json)
        .map_err(anyhow::Error::msg)
}

pub fn dsp_list_types() -> Vec<DspTypeDescriptor> {
    engine().list_dsp_types()
}

pub fn source_list_types() -> Vec<SourceCatalogTypeDescriptor> {
    engine().list_source_catalog_types()
}

pub fn lyrics_provider_list_types() -> Vec<LyricsProviderTypeDescriptor> {
    engine().list_lyrics_provider_types()
}

pub fn output_sink_list_types() -> Vec<OutputSinkTypeDescriptor> {
    engine().list_output_sink_types()
}

pub fn source_list_items_json(
    plugin_id: String,
    type_id: String,
    config_json: String,
    request_json: String,
) -> Result<String> {
    let config = serde_json::from_str::<serde_json::Value>(&config_json)
        .map_err(|e| anyhow!("invalid source config_json: {e}"))?;
    let request = serde_json::from_str::<serde_json::Value>(&request_json)
        .map_err(|e| anyhow!("invalid source request_json: {e}"))?;
    let payload = engine()
        .source_list_items::<serde_json::Value, serde_json::Value, serde_json::Value>(
            &plugin_id, &type_id, &config, &request,
        )
        .map_err(anyhow::Error::msg)?;
    normalize_json_payload("source list response", payload)
}

pub fn lyrics_provider_search_json(
    plugin_id: String,
    type_id: String,
    query_json: String,
) -> Result<String> {
    let query = serde_json::from_str::<serde_json::Value>(&query_json)
        .map_err(|e| anyhow!("invalid lyrics query_json: {e}"))?;
    let payload = engine()
        .lyrics_provider_search::<serde_json::Value, serde_json::Value>(
            &plugin_id, &type_id, &query,
        )
        .map_err(anyhow::Error::msg)?;
    normalize_json_payload("lyrics search response", payload)
}

pub fn lyrics_provider_fetch_json(
    plugin_id: String,
    type_id: String,
    track_json: String,
) -> Result<String> {
    let track = serde_json::from_str::<serde_json::Value>(&track_json)
        .map_err(|e| anyhow!("invalid lyrics track_json: {e}"))?;
    let payload = engine()
        .lyrics_provider_fetch::<serde_json::Value, serde_json::Value>(&plugin_id, &type_id, &track)
        .map_err(anyhow::Error::msg)?;
    normalize_json_payload("lyrics fetch response", payload)
}

pub fn output_sink_list_targets_json(
    plugin_id: String,
    type_id: String,
    config_json: String,
) -> Result<String> {
    let config = serde_json::from_str::<serde_json::Value>(&config_json)
        .map_err(|e| anyhow!("invalid output sink config_json: {e}"))?;
    let payload = engine()
        .output_sink_list_targets::<serde_json::Value, serde_json::Value>(
            &plugin_id, &type_id, &config,
        )
        .map_err(anyhow::Error::msg)?;
    normalize_json_payload("output sink targets", payload)
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
    engine()
        .refresh_devices_async()
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_output_device(backend: AudioBackend, device_id: Option<String>) -> Result<()> {
    engine()
        .set_output_device_async(backend, device_id)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_output_options(
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
) -> Result<()> {
    engine()
        .set_output_options_async(match_track_sample_rate, gapless_playback, seek_track_fade)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn set_output_sink_route(route: OutputSinkRoute) -> Result<()> {
    engine()
        .set_output_sink_route_async(route)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn clear_output_sink_route() -> Result<()> {
    engine()
        .clear_output_sink_route_async()
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn preload_track(path: String, position_ms: u64) -> Result<()> {
    engine()
        .preload_track_ref_async(TrackRef::for_local_path(path), position_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub async fn preload_track_ref(track: TrackRef, position_ms: u64) -> Result<()> {
    engine()
        .preload_track_ref_async(track, position_ms)
        .await
        .map_err(anyhow::Error::msg)
}

pub fn can_play_track_refs(tracks: Vec<TrackRef>) -> Vec<TrackPlayability> {
    engine().can_play_track_refs(tracks)
}

fn normalize_json_payload(label: &str, payload: serde_json::Value) -> Result<String> {
    serde_json::to_string(&payload).map_err(|e| anyhow!("serialize {label}: {e}"))
}
