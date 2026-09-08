use anyhow::Result;

use stellatune_backend_api::runtime::init_tracing;

mod discovery;
mod http_server;
mod metadata;
mod transport;
pub mod types;

use types::{
    DlnaHttpServerInfo, DlnaPositionInfo, DlnaRenderer, DlnaSsdpDevice, DlnaTransportInfo,
};

pub async fn dlna_discover_media_renderers(
    timeout_ms: u32,
) -> Result<Vec<DlnaSsdpDevice>, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::discover_media_renderers(timeout_ms).await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("dlna_discover_media_renderers", error)
    })
}

pub async fn dlna_discover_renderers(
    timeout_ms: u32,
) -> Result<Vec<DlnaRenderer>, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::discover_renderers(timeout_ms).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_discover_renderers", error))
}

pub async fn dlna_http_start(
    advertise_ip: Option<String>,
    port: Option<u16>,
) -> Result<DlnaHttpServerInfo, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::http_start(advertise_ip, port).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_http_start", error))
}

pub async fn dlna_http_publish_track(path: String) -> Result<String, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::http_publish_track(path).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_http_publish_track", error))
}

pub async fn dlna_http_unpublish_all() -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::http_unpublish_all().await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_http_unpublish_all", error))
}

pub async fn dlna_av_transport_set_uri(
    control_url: String,
    service_type: Option<String>,
    uri: String,
    metadata: Option<String>,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::av_transport_set_uri(control_url, service_type, uri, metadata).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_av_transport_set_uri", error))
}

pub async fn dlna_av_transport_play(
    control_url: String,
    service_type: Option<String>,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::av_transport_play(control_url, service_type).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_av_transport_play", error))
}

pub async fn dlna_av_transport_pause(
    control_url: String,
    service_type: Option<String>,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::av_transport_pause(control_url, service_type).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_av_transport_pause", error))
}

pub async fn dlna_av_transport_stop(
    control_url: String,
    service_type: Option<String>,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::av_transport_stop(control_url, service_type).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_av_transport_stop", error))
}

pub async fn dlna_av_transport_seek_ms(
    control_url: String,
    service_type: Option<String>,
    position_ms: u64,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::av_transport_seek_ms(control_url, service_type, position_ms).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_av_transport_seek_ms", error))
}

pub async fn dlna_av_transport_get_transport_info(
    control_url: String,
    service_type: Option<String>,
) -> Result<DlnaTransportInfo, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::av_transport_get_transport_info(control_url, service_type).await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("dlna_av_transport_get_transport_info", error)
    })
}

pub async fn dlna_av_transport_get_position_info(
    control_url: String,
    service_type: Option<String>,
) -> Result<DlnaPositionInfo, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::av_transport_get_position_info(control_url, service_type).await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("dlna_av_transport_get_position_info", error)
    })
}

pub async fn dlna_rendering_control_set_volume(
    control_url: String,
    service_type: Option<String>,
    volume_0_100: u8,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::rendering_control_set_volume(control_url, service_type, volume_0_100).await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("dlna_rendering_control_set_volume", error)
    })
}

pub async fn dlna_rendering_control_set_mute(
    control_url: String,
    service_type: Option<String>,
    mute: bool,
) -> Result<(), crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::rendering_control_set_mute(control_url, service_type, mute).await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("dlna_rendering_control_set_mute", error)
    })
}

pub async fn dlna_rendering_control_get_volume(
    control_url: String,
    service_type: Option<String>,
) -> Result<u8, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::rendering_control_get_volume(control_url, service_type).await
    })
    .await;
    result.map_err(|error| {
        crate::api::error::AppError::capture("dlna_rendering_control_get_volume", error)
    })
}

pub async fn dlna_play_local_path(
    renderer: DlnaRenderer,
    path: String,
) -> Result<String, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::play_local_path(renderer, path).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_play_local_path", error))
}

pub async fn dlna_play_local_track(
    renderer: DlnaRenderer,
    path: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    cover_path: Option<String>,
) -> Result<String, crate::api::error::AppError> {
    let result: anyhow::Result<_> = (async move {
        init_tracing();
        transport::play_local_track(renderer, path, title, artist, album, cover_path).await
    })
    .await;
    result.map_err(|error| crate::api::error::AppError::capture("dlna_play_local_track", error))
}
