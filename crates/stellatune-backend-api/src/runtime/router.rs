use std::sync::OnceLock;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Duration;

use stellatune_core::PluginRuntimeEvent;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use super::router_actor::RuntimeRouterActor;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use super::router_actor::handlers::backend_control::BackendControlMessage;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use super::router_actor::handlers::drain_timeout::DrainTimeoutMessage;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use super::router_actor::handlers::library_event::LibraryEventMessage;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use super::router_actor::handlers::player_event::PlayerEventMessage;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use super::router_actor::handlers::set_engine::SetEngineMessage;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use super::router_actor::handlers::set_library::SetLibraryMessage;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_runtime as global_runtime;

use super::types::{PluginRuntimeEventHub, PluginRuntimeRouter, RouterInbound};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn plugin_runtime_router() -> &'static std::sync::Arc<PluginRuntimeRouter> {
    static ROUTER: OnceLock<std::sync::Arc<PluginRuntimeRouter>> = OnceLock::new();
    ROUTER.get_or_init(|| {
        let (inbound_tx, mut inbound_rx) = tokio::sync::mpsc::unbounded_channel::<RouterInbound>();
        let router = std::sync::Arc::new(PluginRuntimeRouter {
            inbound_tx: inbound_tx.clone(),
            player_event_generation: std::sync::atomic::AtomicU64::new(0),
            library_event_generation: std::sync::atomic::AtomicU64::new(0),
            runtime_hub: std::sync::Arc::new(PluginRuntimeEventHub::new()),
        });

        let (router_actor_ref, _router_actor_join) =
            stellatune_runtime::tokio_actor::spawn_actor(RuntimeRouterActor {
                router: std::sync::Arc::clone(&router),
                engine: None,
                library: None,
                pending_finishes: Vec::new(),
            });

        let backend_actor_ref = router_actor_ref.clone();
        global_runtime::spawn(async move {
            let mut control_rx = stellatune_plugins::runtime::handle::shared_runtime_service()
                .subscribe_backend_control_requests()
                .await;
            while let Some(request) = control_rx.recv().await {
                if backend_actor_ref
                    .cast(BackendControlMessage { request })
                    .is_err()
                {
                    break;
                }
            }
        });

        let inbound_actor_ref = router_actor_ref.clone();
        global_runtime::spawn(async move {
            while let Some(message) = inbound_rx.recv().await {
                let send_result = match message {
                    RouterInbound::SetEngine { engine } => {
                        inbound_actor_ref.cast(SetEngineMessage { engine })
                    }
                    RouterInbound::SetLibrary { library } => {
                        inbound_actor_ref.cast(SetLibraryMessage { library })
                    }
                    RouterInbound::PlayerEvent { generation, event } => {
                        inbound_actor_ref.cast(PlayerEventMessage { generation, event })
                    }
                    RouterInbound::LibraryEvent { generation, event } => {
                        inbound_actor_ref.cast(LibraryEventMessage { generation, event })
                    }
                };
                if send_result.is_err() {
                    break;
                }
            }
        });

        let tick_actor_ref = router_actor_ref;
        global_runtime::spawn(async move {
            let mut timeout_tick = tokio::time::interval(Duration::from_millis(20));
            loop {
                timeout_tick.tick().await;
                if tick_actor_ref.cast(DrainTimeoutMessage).is_err() {
                    break;
                }
            }
        });

        router
    })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn register_plugin_runtime_engine(engine: stellatune_audio::EngineHandle) {
    let router = plugin_runtime_router();
    let mut player_rx = engine.subscribe_events();
    let tx = router.inbound_tx.clone();
    let _ = tx.send(RouterInbound::SetEngine {
        engine: engine.clone(),
    });
    let generation = router
        .player_event_generation
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;
    global_runtime::spawn(async move {
        loop {
            match player_rx.recv().await {
                Ok(event) => {
                    if tx
                        .send(RouterInbound::PlayerEvent { generation, event })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn register_plugin_runtime_library(library: stellatune_library::LibraryHandle) {
    let router = plugin_runtime_router();
    let mut library_rx = library.subscribe_events();
    let tx = router.inbound_tx.clone();
    let _ = tx.send(RouterInbound::SetLibrary {
        library: library.clone(),
    });
    let generation = router
        .library_event_generation
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;
    global_runtime::spawn(async move {
        loop {
            match library_rx.recv().await {
                Ok(event) => {
                    if tx
                        .send(RouterInbound::LibraryEvent { generation, event })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn register_plugin_runtime_engine(_engine: stellatune_audio::EngineHandle) {}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn register_plugin_runtime_library(_library: stellatune_library::LibraryHandle) {}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn subscribe_plugin_runtime_events_global()
-> tokio::sync::broadcast::Receiver<PluginRuntimeEvent> {
    plugin_runtime_router().runtime_hub.subscribe()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn subscribe_plugin_runtime_events_global()
-> tokio::sync::broadcast::Receiver<PluginRuntimeEvent> {
    let (_tx, rx) = tokio::sync::broadcast::channel(1);
    rx
}
