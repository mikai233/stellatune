use crate::api::library::shared_player_service;
use anyhow::{Result, anyhow};
use std::{path::PathBuf, sync::OnceLock};
use stellatune_backend_api::{
    host_api::HostApiHandle,
    runtime::{shared_playback_controller, shared_typescript_runtime},
};
use tokio::sync::Mutex;

fn slot() -> &'static Mutex<Option<HostApiHandle>> {
    static SLOT: OnceLock<Mutex<Option<HostApiHandle>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

pub(super) async fn start(data_root: String) -> Result<String> {
    if data_root.trim().is_empty() {
        return Err(anyhow!("plugin data root is empty"));
    }
    let mut slot = slot().lock().await;
    if let Some(handle) = slot.as_ref() {
        return Ok(handle.base_url());
    }
    let root = PathBuf::from(data_root);
    tokio::fs::create_dir_all(&root).await?;
    let plugins = shared_typescript_runtime();
    let handle = stellatune_backend_api::host_api::start(
        shared_player_service()?,
        shared_playback_controller(),
        plugins.clone(),
    )
    .await?;
    let url = handle.base_url();
    plugins.configure_host(url.clone(), root);
    *slot = Some(handle);
    Ok(url)
}

pub(super) async fn stop() {
    let mut slot = slot().lock().await;
    if let Err(error) = shared_typescript_runtime().shutdown().await {
        tracing::warn!(%error, "plugin shutdown failed");
    }
    if let Some(handle) = slot.take() {
        handle.shutdown().await;
    }
}
