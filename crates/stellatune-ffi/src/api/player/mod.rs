use std::thread;

use crate::frb_generated::{RustOpaque, StreamSink};
use crate::lyrics_service::LyricsService;
use anyhow::Result;

use super::runtime::{
    init_tracing, register_plugin_runtime_engine, shared_plugins,
    subscribe_plugin_runtime_events_global,
};

use stellatune_audio::start_engine_with_plugins;
use stellatune_core::{
    Command, Event, LyricsDoc, LyricsEvent, LyricsQuery, LyricsSearchCandidate, PluginRuntimeEvent,
    TrackDecodeInfo,
};

pub struct Player {
    engine: stellatune_audio::EngineHandle,
    lyrics: std::sync::Arc<LyricsService>,
}

impl Player {
    fn new() -> Self {
        init_tracing();
        tracing::info!("creating player");
        let engine = start_engine_with_plugins(shared_plugins());
        register_plugin_runtime_engine(engine.clone());
        Self {
            engine,
            lyrics: LyricsService::new(),
        }
    }
}

pub fn create_player() -> RustOpaque<Player> {
    RustOpaque::new(Player::new())
}

pub fn load(player: RustOpaque<Player>, path: String) {
    player.engine.send_command(Command::LoadTrackRef {
        track: stellatune_core::TrackRef::for_local_path(path),
    });
}

pub fn load_track_ref(player: RustOpaque<Player>, track: stellatune_core::TrackRef) {
    player.engine.send_command(Command::LoadTrackRef { track });
}

pub fn play(player: RustOpaque<Player>) {
    player.engine.send_command(Command::Play);
}

pub fn pause(player: RustOpaque<Player>) {
    player.engine.send_command(Command::Pause);
}

pub fn seek_ms(player: RustOpaque<Player>, position_ms: u64) {
    player.engine.send_command(Command::SeekMs { position_ms });
}

pub fn set_volume(player: RustOpaque<Player>, volume: f32) {
    player.engine.send_command(Command::SetVolume { volume });
}

pub fn stop(player: RustOpaque<Player>) {
    player.engine.send_command(Command::Stop);
}

pub fn events(player: RustOpaque<Player>, sink: StreamSink<Event>) -> Result<()> {
    let rx = player.engine.subscribe_events();

    thread::Builder::new()
        .name("stellatune-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn stellatune-events thread");

    Ok(())
}

pub fn plugin_runtime_events(
    _player: RustOpaque<Player>,
    sink: StreamSink<PluginRuntimeEvent>,
) -> Result<()> {
    let rx = subscribe_plugin_runtime_events_global();

    thread::Builder::new()
        .name("stellatune-plugin-runtime-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn stellatune-plugin-runtime-events thread");

    Ok(())
}

pub fn plugin_runtime_events_global(sink: StreamSink<PluginRuntimeEvent>) -> Result<()> {
    let rx = subscribe_plugin_runtime_events_global();

    thread::Builder::new()
        .name("stellatune-plugin-runtime-events-global".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn stellatune-plugin-runtime-events-global thread");

    Ok(())
}

pub fn lyrics_prepare(player: RustOpaque<Player>, query: LyricsQuery) -> Result<()> {
    player.lyrics.prepare(query)
}

pub fn lyrics_prefetch(player: RustOpaque<Player>, query: LyricsQuery) -> Result<()> {
    player.lyrics.prefetch(query)
}

pub async fn lyrics_search_candidates(
    player: RustOpaque<Player>,
    query: LyricsQuery,
) -> Result<Vec<LyricsSearchCandidate>> {
    player.lyrics.search_candidates(query).await
}

pub fn lyrics_apply_candidate(
    player: RustOpaque<Player>,
    track_key: String,
    doc: LyricsDoc,
) -> Result<()> {
    player.lyrics.apply_candidate(track_key, doc)
}

pub fn lyrics_set_cache_db_path(player: RustOpaque<Player>, db_path: String) -> Result<()> {
    player.lyrics.set_cache_db_path(db_path)
}

pub fn lyrics_clear_cache(player: RustOpaque<Player>) -> Result<()> {
    player.lyrics.clear_cache()
}

pub fn lyrics_refresh_current(player: RustOpaque<Player>) -> Result<()> {
    player.lyrics.refresh_current()
}

pub fn lyrics_set_position_ms(player: RustOpaque<Player>, position_ms: u64) {
    player.lyrics.set_position_ms(position_ms);
}

pub fn lyrics_events(player: RustOpaque<Player>, sink: StreamSink<LyricsEvent>) -> Result<()> {
    let rx = player.lyrics.subscribe_events();

    thread::Builder::new()
        .name("stellatune-lyrics-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn stellatune-lyrics-events thread");

    Ok(())
}

pub fn plugins_list(player: RustOpaque<Player>) -> Vec<stellatune_core::PluginDescriptor> {
    player.engine.list_plugins()
}

pub fn plugin_publish_event_json(
    player: RustOpaque<Player>,
    plugin_id: Option<String>,
    event_json: String,
) -> Result<()> {
    player
        .engine
        .plugin_publish_event_json(plugin_id, event_json)
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn dsp_list_types(player: RustOpaque<Player>) -> Vec<stellatune_core::DspTypeDescriptor> {
    player.engine.list_dsp_types()
}

pub fn source_list_types(
    player: RustOpaque<Player>,
) -> Vec<stellatune_core::SourceCatalogTypeDescriptor> {
    player.engine.list_source_catalog_types()
}

pub fn lyrics_provider_list_types(
    player: RustOpaque<Player>,
) -> Vec<stellatune_core::LyricsProviderTypeDescriptor> {
    player.engine.list_lyrics_provider_types()
}

pub fn output_sink_list_types(
    player: RustOpaque<Player>,
) -> Vec<stellatune_core::OutputSinkTypeDescriptor> {
    player.engine.list_output_sink_types()
}

pub fn source_list_items_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    config_json: String,
    request_json: String,
) -> Result<String> {
    player
        .engine
        .source_list_items_json(&plugin_id, &type_id, &config_json, &request_json)
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn lyrics_provider_search_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    query_json: String,
) -> Result<String> {
    player
        .engine
        .lyrics_provider_search_json(&plugin_id, &type_id, &query_json)
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn lyrics_provider_fetch_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    track_json: String,
) -> Result<String> {
    player
        .engine
        .lyrics_provider_fetch_json(&plugin_id, &type_id, &track_json)
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn output_sink_list_targets_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    config_json: String,
) -> Result<String> {
    player
        .engine
        .output_sink_list_targets_json(&plugin_id, &type_id, &config_json)
        .map_err(|e| anyhow::anyhow!(e))
}

pub fn dsp_set_chain(player: RustOpaque<Player>, chain: Vec<stellatune_core::DspChainItem>) {
    player.engine.set_dsp_chain(chain);
}

pub fn current_track_info(player: RustOpaque<Player>) -> Option<TrackDecodeInfo> {
    player.engine.current_track_info()
}

pub fn plugins_reload(player: RustOpaque<Player>, dir: String) {
    player.engine.reload_plugins(dir);
}

pub fn plugins_reload_with_disabled(
    player: RustOpaque<Player>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    player
        .engine
        .reload_plugins_with_disabled(dir, disabled_ids);
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

pub fn refresh_devices(player: RustOpaque<Player>) {
    player.engine.send_command(Command::RefreshDevices);
}

pub fn set_output_device(
    player: RustOpaque<Player>,
    backend: stellatune_core::AudioBackend,
    device_id: Option<String>,
) {
    player
        .engine
        .send_command(Command::SetOutputDevice { backend, device_id });
}

pub fn set_output_options(
    player: RustOpaque<Player>,
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
) {
    player.engine.send_command(Command::SetOutputOptions {
        match_track_sample_rate,
        gapless_playback,
        seek_track_fade,
    });
}

pub fn set_output_sink_route(player: RustOpaque<Player>, route: stellatune_core::OutputSinkRoute) {
    player
        .engine
        .send_command(Command::SetOutputSinkRoute { route });
}

pub fn clear_output_sink_route(player: RustOpaque<Player>) {
    player.engine.send_command(Command::ClearOutputSinkRoute);
}

pub fn preload_track(player: RustOpaque<Player>, path: String, position_ms: u64) {
    player.engine.send_command(Command::PreloadTrackRef {
        track: stellatune_core::TrackRef::for_local_path(path),
        position_ms,
    });
}

pub fn preload_track_ref(
    player: RustOpaque<Player>,
    track: stellatune_core::TrackRef,
    position_ms: u64,
) {
    player
        .engine
        .send_command(Command::PreloadTrackRef { track, position_ms });
}
