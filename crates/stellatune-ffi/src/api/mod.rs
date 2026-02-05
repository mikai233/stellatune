use std::sync::OnceLock;
use std::thread;

use crate::frb_generated::{RustOpaque, StreamSink};
use anyhow::Result;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::LocalTime;

use stellatune_audio::start_engine;
use stellatune_core::{
    Command, DlnaHttpServerInfo, DlnaPositionInfo, DlnaRenderer, DlnaSsdpDevice, DlnaTransportInfo,
    Event, LibraryCommand, LibraryEvent, TrackDecodeInfo,
};
use stellatune_library::start_library;

mod dlna;

fn init_tracing() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // Default to debug logs in debug builds to make performance profiling easier.
            // Users can always override via `RUST_LOG=...`.
            if cfg!(debug_assertions) {
                EnvFilter::new(
                    // Keep StellaTune crates at debug for easier profiling/diagnostics.
                    "warn,stellatune_ffi=debug,stellatune_audio=debug,stellatune_decode=debug,stellatune_output=debug,stellatune_library=debug,stellatune_plugins=debug",
                )
            } else {
                EnvFilter::new("info")
            }
        });
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_timer(LocalTime::rfc_3339())
            // Include log targets so it's easier to see which crate/module emits a message
            // (e.g. Symphonia demuxer warnings during probing).
            .with_target(true)
            .with_thread_names(true)
            .with_thread_ids(true)
            .init();
    });
}

pub struct Player {
    engine: stellatune_audio::EngineHandle,
}

impl Player {
    fn new() -> Self {
        init_tracing();
        tracing::info!("creating player");
        Self {
            engine: start_engine(),
        }
    }
}

pub fn create_player() -> RustOpaque<Player> {
    RustOpaque::new(Player::new())
}

pub fn load(player: RustOpaque<Player>, path: String) {
    player.engine.send_command(Command::LoadTrack { path });
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

pub fn plugins_list(player: RustOpaque<Player>) -> Vec<stellatune_core::PluginDescriptor> {
    player.engine.list_plugins()
}

pub fn dsp_list_types(player: RustOpaque<Player>) -> Vec<stellatune_core::DspTypeDescriptor> {
    player.engine.list_dsp_types()
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

pub fn refresh_devices(player: RustOpaque<Player>) {
    player.engine.send_command(Command::RefreshDevices);
}

pub fn set_output_device(
    player: RustOpaque<Player>,
    backend: stellatune_core::AudioBackend,
    device_name: Option<String>,
) {
    player.engine.send_command(Command::SetOutputDevice {
        backend,
        device_name,
    });
}

pub struct Library {
    handle: stellatune_library::LibraryHandle,
}

impl Library {
    fn new(db_path: String, disabled_plugin_ids: Vec<String>) -> Result<Self> {
        init_tracing();
        tracing::info!("creating library: {}", db_path);
        Ok(Self {
            handle: start_library(db_path, disabled_plugin_ids)?,
        })
    }
}

pub fn create_library(
    db_path: String,
    disabled_plugin_ids: Vec<String>,
) -> Result<RustOpaque<Library>> {
    Ok(RustOpaque::new(Library::new(db_path, disabled_plugin_ids)?))
}

pub fn library_add_root(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::AddRoot { path });
}

pub fn library_remove_root(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::RemoveRoot { path });
}

pub fn library_delete_folder(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::DeleteFolder { path });
}

pub fn library_restore_folder(library: RustOpaque<Library>, path: String) {
    library
        .handle
        .send_command(LibraryCommand::RestoreFolder { path });
}

pub fn library_list_excluded_folders(library: RustOpaque<Library>) {
    library
        .handle
        .send_command(LibraryCommand::ListExcludedFolders);
}

pub fn library_scan_all(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ScanAll);
}

pub fn library_scan_all_force(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ScanAllForce);
}

pub fn library_list_roots(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ListRoots);
}

pub fn library_list_folders(library: RustOpaque<Library>) {
    library.handle.send_command(LibraryCommand::ListFolders);
}

pub fn library_list_tracks(
    library: RustOpaque<Library>,
    folder: String,
    recursive: bool,
    query: String,
    limit: i64,
    offset: i64,
) {
    library.handle.send_command(LibraryCommand::ListTracks {
        folder,
        recursive,
        query,
        limit,
        offset,
    });
}

pub fn library_search(library: RustOpaque<Library>, query: String, limit: i64, offset: i64) {
    library.handle.send_command(LibraryCommand::Search {
        query,
        limit,
        offset,
    });
}

pub fn library_events(library: RustOpaque<Library>, sink: StreamSink<LibraryEvent>) -> Result<()> {
    let rx = library.handle.subscribe_events();

    thread::Builder::new()
        .name("stellatune-library-events".to_string())
        .spawn(move || {
            for event in rx.iter() {
                if sink.add(event).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn stellatune-library-events thread");

    Ok(())
}

pub fn library_plugins_reload_with_disabled(
    library: RustOpaque<Library>,
    dir: String,
    disabled_ids: Vec<String>,
) {
    library
        .handle
        .plugins_reload_with_disabled(dir, disabled_ids);
}

pub async fn dlna_discover_media_renderers(timeout_ms: u32) -> Result<Vec<DlnaSsdpDevice>> {
    init_tracing();
    dlna::Dlna::discover_media_renderers(timeout_ms).await
}

pub async fn dlna_discover_renderers(timeout_ms: u32) -> Result<Vec<DlnaRenderer>> {
    init_tracing();
    dlna::Dlna::discover_renderers(timeout_ms).await
}

pub async fn dlna_http_start(
    advertise_ip: Option<String>,
    port: Option<u16>,
) -> Result<DlnaHttpServerInfo> {
    init_tracing();
    dlna::Dlna::http_start(advertise_ip, port).await
}

pub async fn dlna_http_publish_track(path: String) -> Result<String> {
    init_tracing();
    dlna::Dlna::http_publish_track(path).await
}

pub async fn dlna_http_unpublish_all() -> Result<()> {
    init_tracing();
    dlna::Dlna::http_unpublish_all().await
}

pub async fn dlna_av_transport_set_uri(
    control_url: String,
    service_type: Option<String>,
    uri: String,
    metadata: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna::Dlna::av_transport_set_uri(control_url, service_type, uri, metadata).await
}

pub async fn dlna_av_transport_play(
    control_url: String,
    service_type: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna::Dlna::av_transport_play(control_url, service_type).await
}

pub async fn dlna_av_transport_pause(
    control_url: String,
    service_type: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna::Dlna::av_transport_pause(control_url, service_type).await
}

pub async fn dlna_av_transport_stop(
    control_url: String,
    service_type: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna::Dlna::av_transport_stop(control_url, service_type).await
}

pub async fn dlna_av_transport_seek_ms(
    control_url: String,
    service_type: Option<String>,
    position_ms: u64,
) -> Result<()> {
    init_tracing();
    dlna::Dlna::av_transport_seek_ms(control_url, service_type, position_ms).await
}

pub async fn dlna_av_transport_get_transport_info(
    control_url: String,
    service_type: Option<String>,
) -> Result<DlnaTransportInfo> {
    init_tracing();
    dlna::Dlna::av_transport_get_transport_info(control_url, service_type).await
}

pub async fn dlna_av_transport_get_position_info(
    control_url: String,
    service_type: Option<String>,
) -> Result<DlnaPositionInfo> {
    init_tracing();
    dlna::Dlna::av_transport_get_position_info(control_url, service_type).await
}

pub async fn dlna_rendering_control_set_volume(
    control_url: String,
    service_type: Option<String>,
    volume_0_100: u8,
) -> Result<()> {
    init_tracing();
    dlna::Dlna::rendering_control_set_volume(control_url, service_type, volume_0_100).await
}

pub async fn dlna_rendering_control_set_mute(
    control_url: String,
    service_type: Option<String>,
    mute: bool,
) -> Result<()> {
    init_tracing();
    dlna::Dlna::rendering_control_set_mute(control_url, service_type, mute).await
}

pub async fn dlna_rendering_control_get_volume(
    control_url: String,
    service_type: Option<String>,
) -> Result<u8> {
    init_tracing();
    dlna::Dlna::rendering_control_get_volume(control_url, service_type).await
}

pub async fn dlna_play_local_path(renderer: DlnaRenderer, path: String) -> Result<String> {
    init_tracing();
    dlna::Dlna::play_local_path(renderer, path).await
}

pub async fn dlna_play_local_track(
    renderer: DlnaRenderer,
    path: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    cover_path: Option<String>,
) -> Result<String> {
    init_tracing();
    dlna::Dlna::play_local_track(renderer, path, title, artist, album, cover_path).await
}
