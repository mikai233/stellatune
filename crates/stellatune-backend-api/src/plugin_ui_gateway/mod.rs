mod auth;
mod dev;
mod handlers;
mod model;
mod permissions;
mod runtime_apply;
mod state;
mod storage;

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use axum::Router;
use axum::middleware;
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use state::GatewayState;

#[derive(Debug, Clone)]
pub struct PluginUiGatewayOptions {
    pub plugins_dir: PathBuf,
    pub bind_addr: SocketAddr,
    pub dev_ui_origins: HashMap<String, String>,
}

impl PluginUiGatewayOptions {
    pub fn localhost_random_port(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
            bind_addr: SocketAddr::from((IpAddr::V4(Ipv4Addr::LOCALHOST), 0)),
            dev_ui_origins: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct PluginUiGatewayHandle {
    local_addr: SocketAddr,
    session_token: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl PluginUiGatewayHandle {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.local_addr)
    }

    pub fn session_token(&self) -> &str {
        &self.session_token
    }

    pub fn plugin_ui_url(&self, plugin_id: &str) -> Option<String> {
        let plugin_id = plugin_id.trim();
        if !is_safe_plugin_id(plugin_id) {
            return None;
        }
        Some(format!(
            "{}/ui/{plugin_id}/?token={}",
            self.base_url(),
            self.session_token
        ))
    }

    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.await
                .map_err(|error| anyhow!("plugin ui gateway task join failed: {error}"))?;
        }
        Ok(())
    }
}

impl Drop for PluginUiGatewayHandle {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(join) = self.join.take() {
            join.abort();
        }
    }
}

pub async fn start_plugin_ui_gateway(
    options: PluginUiGatewayOptions,
) -> Result<PluginUiGatewayHandle> {
    if options.plugins_dir.as_os_str().is_empty() {
        return Err(anyhow!("plugins_dir must not be empty"));
    }

    tokio::fs::create_dir_all(&options.plugins_dir)
        .await
        .with_context(|| format!("create plugins dir {}", options.plugins_dir.display()))?;

    let listener = TcpListener::bind(options.bind_addr)
        .await
        .with_context(|| format!("bind plugin ui gateway on {}", options.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("resolve plugin ui gateway local addr")?;
    let session_token = auth::generate_session_token();
    let dev_ui_overrides = dev::merge_dev_ui_overrides(options.dev_ui_origins)?;
    let mut allowed_origins = auth::build_allowed_origins(local_addr);
    allowed_origins.extend(dev_ui_overrides.values().cloned());
    let gateway_origin = format!("http://{}", local_addr);

    let state = GatewayState {
        plugins_dir: options.plugins_dir,
        event_bus: Default::default(),
        session_token: std::sync::Arc::from(session_token.as_str()),
        allowed_origins: std::sync::Arc::new(allowed_origins),
        dev_ui_overrides: std::sync::Arc::new(dev_ui_overrides),
        gateway_origin: std::sync::Arc::from(gateway_origin.as_str()),
    };
    let api_state = state.clone();
    let api_routes = Router::new()
        .route(
            "/api/plugins/{plugin_id}/config",
            get(handlers::get_plugin_config)
                .put(handlers::put_plugin_config)
                .options(handlers::api_preflight),
        )
        .route(
            "/api/plugins/{plugin_id}/actions/{action}",
            post(handlers::invoke_plugin_action).options(handlers::api_preflight),
        )
        .route(
            "/api/plugins/{plugin_id}/events",
            get(handlers::stream_plugin_events).options(handlers::api_preflight),
        )
        .route_layer(middleware::from_fn_with_state(
            api_state,
            auth::require_api_access,
        ));

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/ui/{plugin_id}", get(handlers::redirect_plugin_ui_index))
        .route("/ui/{plugin_id}/", get(handlers::serve_plugin_ui_index))
        .route(
            "/ui/{plugin_id}/{*path}",
            get(handlers::serve_plugin_ui_asset),
        )
        .merge(api_routes)
        .with_state(state);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let join = tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        if let Err(error) = server.await {
            tracing::warn!(
                target: "stellatune_backend_api::plugin_ui_gateway",
                %error,
                "plugin ui gateway stopped with server error"
            );
        }
    });

    Ok(PluginUiGatewayHandle {
        local_addr,
        session_token,
        shutdown_tx: Some(shutdown_tx),
        join: Some(join),
    })
}

fn is_safe_plugin_id(plugin_id: &str) -> bool {
    dev::is_safe_plugin_id(plugin_id)
}
