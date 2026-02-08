use anyhow::Result;

use super::dlna_impl;
use super::runtime::init_tracing;

use stellatune_core::{
    DlnaHttpServerInfo, DlnaPositionInfo, DlnaRenderer, DlnaSsdpDevice, DlnaTransportInfo,
};

pub async fn dlna_discover_media_renderers(timeout_ms: u32) -> Result<Vec<DlnaSsdpDevice>> {
    init_tracing();
    dlna_impl::Dlna::discover_media_renderers(timeout_ms).await
}

pub async fn dlna_discover_renderers(timeout_ms: u32) -> Result<Vec<DlnaRenderer>> {
    init_tracing();
    dlna_impl::Dlna::discover_renderers(timeout_ms).await
}

pub async fn dlna_http_start(
    advertise_ip: Option<String>,
    port: Option<u16>,
) -> Result<DlnaHttpServerInfo> {
    init_tracing();
    dlna_impl::Dlna::http_start(advertise_ip, port).await
}

pub async fn dlna_http_publish_track(path: String) -> Result<String> {
    init_tracing();
    dlna_impl::Dlna::http_publish_track(path).await
}

pub async fn dlna_http_unpublish_all() -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::http_unpublish_all().await
}

pub async fn dlna_av_transport_set_uri(
    control_url: String,
    service_type: Option<String>,
    uri: String,
    metadata: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::av_transport_set_uri(control_url, service_type, uri, metadata).await
}

pub async fn dlna_av_transport_play(
    control_url: String,
    service_type: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::av_transport_play(control_url, service_type).await
}

pub async fn dlna_av_transport_pause(
    control_url: String,
    service_type: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::av_transport_pause(control_url, service_type).await
}

pub async fn dlna_av_transport_stop(
    control_url: String,
    service_type: Option<String>,
) -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::av_transport_stop(control_url, service_type).await
}

pub async fn dlna_av_transport_seek_ms(
    control_url: String,
    service_type: Option<String>,
    position_ms: u64,
) -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::av_transport_seek_ms(control_url, service_type, position_ms).await
}

pub async fn dlna_av_transport_get_transport_info(
    control_url: String,
    service_type: Option<String>,
) -> Result<DlnaTransportInfo> {
    init_tracing();
    dlna_impl::Dlna::av_transport_get_transport_info(control_url, service_type).await
}

pub async fn dlna_av_transport_get_position_info(
    control_url: String,
    service_type: Option<String>,
) -> Result<DlnaPositionInfo> {
    init_tracing();
    dlna_impl::Dlna::av_transport_get_position_info(control_url, service_type).await
}

pub async fn dlna_rendering_control_set_volume(
    control_url: String,
    service_type: Option<String>,
    volume_0_100: u8,
) -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::rendering_control_set_volume(control_url, service_type, volume_0_100).await
}

pub async fn dlna_rendering_control_set_mute(
    control_url: String,
    service_type: Option<String>,
    mute: bool,
) -> Result<()> {
    init_tracing();
    dlna_impl::Dlna::rendering_control_set_mute(control_url, service_type, mute).await
}

pub async fn dlna_rendering_control_get_volume(
    control_url: String,
    service_type: Option<String>,
) -> Result<u8> {
    init_tracing();
    dlna_impl::Dlna::rendering_control_get_volume(control_url, service_type).await
}

pub async fn dlna_play_local_path(renderer: DlnaRenderer, path: String) -> Result<String> {
    init_tracing();
    dlna_impl::Dlna::play_local_path(renderer, path).await
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
    dlna_impl::Dlna::play_local_track(renderer, path, title, artist, album, cover_path).await
}
