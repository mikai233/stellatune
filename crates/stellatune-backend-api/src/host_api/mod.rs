//! Local player control transport. Plugin pages and business routes live in Node.
mod error;
mod handlers;
pub mod model;

use crate::player_service::service::PlayerService;
use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use stellatune_audio::playback::control::PlaybackController;
use stellatune_plugins::typescript::TypeScriptRuntime;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct HostApiState {
    pub service: Arc<PlayerService>,
    pub controller: PlaybackController,
    pub plugins: Arc<TypeScriptRuntime>,
    pub(super) shutdown: CancellationToken,
}

pub struct HostApiHandle {
    address: SocketAddr,
    shutdown: CancellationToken,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl HostApiHandle {
    pub fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }
    pub async fn shutdown(mut self) {
        self.shutdown.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}
impl Drop for HostApiHandle {
    fn drop(&mut self) {
        self.shutdown.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Starts the transport. The application owns service restoration and starts
/// its state writer after plugins have been registered and playback restored.
pub async fn start(
    service: Arc<PlayerService>,
    controller: PlaybackController,
    plugins: Arc<TypeScriptRuntime>,
) -> anyhow::Result<HostApiHandle> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let address = listener.local_addr()?;
    let shutdown = CancellationToken::new();
    let state = HostApiState {
        service,
        controller,
        plugins,
        shutdown: shutdown.clone(),
    };
    let app = router(state);
    let stopped = shutdown.clone();
    let task = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(stopped.cancelled_owned())
            .await
        {
            tracing::warn!(%error, "host API stopped with an error");
        }
    });
    Ok(HostApiHandle {
        address,
        shutdown,
        task: Some(task),
    })
}

fn router(state: HostApiState) -> Router {
    Router::new()
        .route(
            "/health",
            get(|| async { axum::Json(serde_json::json!({"ok": true})) }),
        )
        .route("/player/state", get(handlers::state))
        .route("/player/queue", get(handlers::queue))
        .route("/player/commands", post(handlers::command))
        .route("/player/events", get(handlers::events))
        .layer(middleware::from_fn(cors))
        .with_state(state)
}

async fn cors(request: Request, next: Next) -> Response {
    let mut response = if request.method() == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET,POST,OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("*"),
    );
    response
}
