use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use tokio::sync::Mutex;

use stellatune_backend_api::plugin_ui_gateway::{
    PluginUiGatewayHandle, PluginUiGatewayOptions, start_plugin_ui_gateway,
};
use stellatune_backend_api::runtime::init_tracing;

fn shared_gateway_slot() -> &'static Mutex<Option<PluginUiGatewayHandle>> {
    static SLOT: OnceLock<Mutex<Option<PluginUiGatewayHandle>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn normalize_plugins_dir(plugins_dir: String) -> Result<PathBuf> {
    let trimmed = plugins_dir.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("plugins_dir is empty"));
    }
    Ok(PathBuf::from(trimmed))
}

fn make_options(plugins_dir: PathBuf, port: Option<u16>) -> PluginUiGatewayOptions {
    match port {
        Some(p) => PluginUiGatewayOptions {
            plugins_dir,
            bind_addr: SocketAddr::from((IpAddr::V4(Ipv4Addr::LOCALHOST), p)),
            dev_ui_origins: std::collections::HashMap::new(),
        },
        None => PluginUiGatewayOptions::localhost_random_port(plugins_dir),
    }
}

pub(crate) async fn start(plugins_dir: String, port: Option<u16>) -> Result<String> {
    init_tracing();
    let plugins_dir = normalize_plugins_dir(plugins_dir)?;
    let options = make_options(plugins_dir, port);
    let handle = start_plugin_ui_gateway(options).await?;
    let base_url = handle.base_url();

    let mut slot = shared_gateway_slot().lock().await;
    if let Some(previous) = slot.take() {
        drop(slot);
        previous.shutdown().await?;
        slot = shared_gateway_slot().lock().await;
    }
    *slot = Some(handle);
    Ok(base_url)
}

pub(crate) async fn stop() -> Result<()> {
    init_tracing();
    let mut slot = shared_gateway_slot().lock().await;
    let handle = slot.take();
    drop(slot);
    if let Some(handle) = handle {
        handle.shutdown().await?;
    }
    Ok(())
}

pub(crate) async fn base_url() -> Option<String> {
    let slot = shared_gateway_slot().lock().await;
    slot.as_ref().map(PluginUiGatewayHandle::base_url)
}

pub(crate) async fn session_token() -> Option<String> {
    let slot = shared_gateway_slot().lock().await;
    slot.as_ref()
        .map(|handle| handle.session_token().to_string())
}

pub(crate) async fn plugin_ui_url(plugin_id: String) -> Option<String> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return None;
    }

    let slot = shared_gateway_slot().lock().await;
    slot.as_ref()
        .and_then(|handle| handle.plugin_ui_url(plugin_id))
}
