use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use tokio::sync::{Mutex, broadcast};

use crate::plugin_ui_gateway::model::PluginUiEvent;

pub(super) type HttpError = (StatusCode, String);
pub(super) type HttpResult<T> = std::result::Result<T, HttpError>;

#[derive(Debug, Clone)]
pub(super) struct GatewayState {
    pub(super) plugins_dir: PathBuf,
    pub(super) event_bus: EventBus,
    pub(super) session_token: Arc<str>,
    pub(super) allowed_origins: Arc<HashSet<String>>,
    pub(super) dev_ui_overrides: Arc<HashMap<String, String>>,
    pub(super) gateway_origin: Arc<str>,
}

impl GatewayState {
    pub(super) fn session_token(&self) -> &str {
        &self.session_token
    }

    pub(super) fn is_origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins.contains(origin)
    }

    pub(super) fn dev_origin_for_plugin(&self, plugin_id: &str) -> Option<&str> {
        self.dev_ui_overrides.get(plugin_id).map(String::as_str)
    }

    pub(super) fn gateway_origin(&self) -> &str {
        &self.gateway_origin
    }

    pub(super) async fn subscribe_plugin_events(
        &self,
        plugin_id: &str,
    ) -> broadcast::Receiver<PluginUiEvent> {
        self.event_bus.subscribe(plugin_id).await
    }

    pub(super) async fn publish_plugin_event(
        &self,
        plugin_id: &str,
        name: impl Into<String>,
        payload: serde_json::Value,
    ) {
        self.event_bus
            .publish(plugin_id, name.into(), payload, now_unix_ms())
            .await;
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct EventBus {
    channels: Arc<Mutex<HashMap<String, broadcast::Sender<PluginUiEvent>>>>,
}

impl EventBus {
    async fn subscribe(&self, plugin_id: &str) -> broadcast::Receiver<PluginUiEvent> {
        let plugin_id = plugin_id.trim().to_string();
        let mut channels = self.channels.lock().await;
        let sender = channels
            .entry(plugin_id)
            .or_insert_with(|| broadcast::channel::<PluginUiEvent>(128).0);
        sender.subscribe()
    }

    async fn publish(&self, plugin_id: &str, name: String, payload: serde_json::Value, ts_ms: u64) {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return;
        }
        let event = PluginUiEvent {
            plugin_id: plugin_id.clone(),
            name,
            payload,
            ts_ms,
        };
        let mut channels = self.channels.lock().await;
        let sender = channels
            .entry(plugin_id)
            .or_insert_with(|| broadcast::channel::<PluginUiEvent>(128).0);
        let _ = sender.send(event);
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}
