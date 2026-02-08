use std::thread;

use crate::frb_generated::{RustOpaque, StreamSink};
use anyhow::Result;

use stellatune_backend_api::player::{
    PlayerService, plugins_install_from_file as backend_plugins_install_from_file,
    plugins_list_installed_json as backend_plugins_list_installed_json,
    plugins_uninstall_by_id as backend_plugins_uninstall_by_id,
};
use stellatune_core::{
    AudioBackend, DspChainItem, DspTypeDescriptor, Event, LyricsDoc, LyricsEvent,
    LyricsProviderTypeDescriptor, LyricsQuery, LyricsSearchCandidate, OutputSinkRoute,
    OutputSinkTypeDescriptor, PluginDescriptor, PluginRuntimeEvent, SourceCatalogTypeDescriptor,
    TrackDecodeInfo, TrackRef,
};

pub struct Player {
    service: PlayerService,
}

impl Player {
    fn new() -> Self {
        Self {
            service: PlayerService::new(),
        }
    }
}

pub fn create_player() -> RustOpaque<Player> {
    RustOpaque::new(Player::new())
}

pub fn load(player: RustOpaque<Player>, path: String) {
    player.service.load(path);
}

pub fn load_track_ref(player: RustOpaque<Player>, track: TrackRef) {
    player.service.load_track_ref(track);
}

pub fn play(player: RustOpaque<Player>) {
    player.service.play();
}

pub fn pause(player: RustOpaque<Player>) {
    player.service.pause();
}

pub fn seek_ms(player: RustOpaque<Player>, position_ms: u64) {
    player.service.seek_ms(position_ms);
}

pub fn set_volume(player: RustOpaque<Player>, volume: f32) {
    player.service.set_volume(volume);
}

pub fn stop(player: RustOpaque<Player>) {
    player.service.stop();
}

pub fn events(player: RustOpaque<Player>, sink: StreamSink<Event>) -> Result<()> {
    let rx = player.service.subscribe_events();

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
    player: RustOpaque<Player>,
    sink: StreamSink<PluginRuntimeEvent>,
) -> Result<()> {
    let rx = player.service.subscribe_plugin_runtime_events();

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
    let rx = stellatune_backend_api::runtime::subscribe_plugin_runtime_events_global();

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
    player.service.lyrics_prepare(query)
}

pub fn lyrics_prefetch(player: RustOpaque<Player>, query: LyricsQuery) -> Result<()> {
    player.service.lyrics_prefetch(query)
}

pub async fn lyrics_search_candidates(
    player: RustOpaque<Player>,
    query: LyricsQuery,
) -> Result<Vec<LyricsSearchCandidate>> {
    player.service.lyrics_search_candidates(query).await
}

pub fn lyrics_apply_candidate(
    player: RustOpaque<Player>,
    track_key: String,
    doc: LyricsDoc,
) -> Result<()> {
    player.service.lyrics_apply_candidate(track_key, doc)
}

pub fn lyrics_set_cache_db_path(player: RustOpaque<Player>, db_path: String) -> Result<()> {
    player.service.lyrics_set_cache_db_path(db_path)
}

pub fn lyrics_clear_cache(player: RustOpaque<Player>) -> Result<()> {
    player.service.lyrics_clear_cache()
}

pub fn lyrics_refresh_current(player: RustOpaque<Player>) -> Result<()> {
    player.service.lyrics_refresh_current()
}

pub fn lyrics_set_position_ms(player: RustOpaque<Player>, position_ms: u64) {
    player.service.lyrics_set_position_ms(position_ms);
}

pub fn lyrics_events(player: RustOpaque<Player>, sink: StreamSink<LyricsEvent>) -> Result<()> {
    let rx = player.service.subscribe_lyrics_events();

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

pub fn plugins_list(player: RustOpaque<Player>) -> Vec<PluginDescriptor> {
    player.service.plugins_list()
}

pub fn plugin_publish_event_json(
    player: RustOpaque<Player>,
    plugin_id: Option<String>,
    event_json: String,
) -> Result<()> {
    player
        .service
        .plugin_publish_event_json(plugin_id, event_json)
}

pub fn dsp_list_types(player: RustOpaque<Player>) -> Vec<DspTypeDescriptor> {
    player.service.dsp_list_types()
}

pub fn source_list_types(player: RustOpaque<Player>) -> Vec<SourceCatalogTypeDescriptor> {
    player.service.source_list_types()
}

pub fn lyrics_provider_list_types(player: RustOpaque<Player>) -> Vec<LyricsProviderTypeDescriptor> {
    player.service.lyrics_provider_list_types()
}

pub fn output_sink_list_types(player: RustOpaque<Player>) -> Vec<OutputSinkTypeDescriptor> {
    player.service.output_sink_list_types()
}

pub fn source_list_items_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    config_json: String,
    request_json: String,
) -> Result<String> {
    player
        .service
        .source_list_items_json(plugin_id, type_id, config_json, request_json)
}

pub fn lyrics_provider_search_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    query_json: String,
) -> Result<String> {
    player
        .service
        .lyrics_provider_search_json(plugin_id, type_id, query_json)
}

pub fn lyrics_provider_fetch_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    track_json: String,
) -> Result<String> {
    player
        .service
        .lyrics_provider_fetch_json(plugin_id, type_id, track_json)
}

pub fn output_sink_list_targets_json(
    player: RustOpaque<Player>,
    plugin_id: String,
    type_id: String,
    config_json: String,
) -> Result<String> {
    player
        .service
        .output_sink_list_targets_json(plugin_id, type_id, config_json)
}

pub fn dsp_set_chain(player: RustOpaque<Player>, chain: Vec<DspChainItem>) {
    player.service.dsp_set_chain(chain);
}

pub fn current_track_info(player: RustOpaque<Player>) -> Option<TrackDecodeInfo> {
    player.service.current_track_info()
}

pub fn plugins_reload(player: RustOpaque<Player>, dir: String) {
    player.service.plugins_reload(dir);
}

pub fn plugins_reload_with_disabled(
    player: RustOpaque<Player>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    player
        .service
        .plugins_reload_with_disabled(dir, disabled_ids);
}

pub fn plugins_install_from_file(plugins_dir: String, artifact_path: String) -> Result<String> {
    backend_plugins_install_from_file(plugins_dir, artifact_path)
}

pub fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    backend_plugins_list_installed_json(plugins_dir)
}

pub fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    backend_plugins_uninstall_by_id(plugins_dir, plugin_id)
}

pub fn refresh_devices(player: RustOpaque<Player>) {
    player.service.refresh_devices();
}

pub fn set_output_device(
    player: RustOpaque<Player>,
    backend: AudioBackend,
    device_id: Option<String>,
) {
    player.service.set_output_device(backend, device_id);
}

pub fn set_output_options(
    player: RustOpaque<Player>,
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
) {
    player
        .service
        .set_output_options(match_track_sample_rate, gapless_playback, seek_track_fade);
}

pub fn set_output_sink_route(player: RustOpaque<Player>, route: OutputSinkRoute) {
    player.service.set_output_sink_route(route);
}

pub fn clear_output_sink_route(player: RustOpaque<Player>) {
    player.service.clear_output_sink_route();
}

pub fn preload_track(player: RustOpaque<Player>, path: String, position_ms: u64) {
    player.service.preload_track(path, position_ms);
}

pub fn preload_track_ref(player: RustOpaque<Player>, track: TrackRef, position_ms: u64) {
    player.service.preload_track_ref(track, position_ms);
}
