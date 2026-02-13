use std::sync::OnceLock;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Duration;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Instant;

use stellatune_core::{
    Event, HostEventTopic, HostLibraryEventEnvelope, HostPlayerEventEnvelope, LibraryEvent,
    PluginRuntimeEvent, PluginRuntimeKind,
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugin_protocol::PluginControlRequest;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugins::runtime::backend_control::{BackendControlRequest, BackendControlResponse};

use super::bus::{
    ControlFinishedArgs, broadcast_host_event_json, build_control_result_payload,
    drain_finished_by_library_event, drain_finished_by_player_event, drain_timed_out_pending,
    emit_control_finished, emit_runtime_event, push_plugin_host_event_json,
};
use super::control::{control_wait_kind, route_plugin_control_request};
use super::types::{
    CONTROL_FINISH_TIMEOUT, ControlWaitKind, PendingControlFinish, PluginRuntimeEventHub,
    PluginRuntimeRouter, RouterInbound,
};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_runtime as global_runtime;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn should_broadcast_player_event(event: &Event) -> bool {
    !matches!(event, Event::Position { .. } | Event::Log { .. })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn should_broadcast_library_event(event: &LibraryEvent) -> bool {
    !matches!(event, LibraryEvent::Log { .. })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
async fn handle_backend_control_request(
    router: &PluginRuntimeRouter,
    pending_finishes: &mut Vec<PendingControlFinish>,
    request: BackendControlRequest,
    engine: Option<&stellatune_audio::EngineHandle>,
    library: Option<&stellatune_library::LibraryHandle>,
) {
    let parsed_request = match serde_json::from_str::<PluginControlRequest>(&request.request_json) {
        Ok(parsed) => Some(parsed),
        Err(err) => {
            let error = format!("invalid json: {err}");
            let _ = request
                .response_tx
                .send(BackendControlResponse::error(-1, error));
            None
        }
    };

    let route_result = match parsed_request.as_ref() {
        Some(parsed) => route_plugin_control_request(parsed, engine, library).await,
        None => Err("invalid control request json".to_string()),
    };

    let payload = build_control_result_payload(
        parsed_request.as_ref(),
        route_result.as_ref().err().map(String::as_str),
    );
    let response_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());

    let response = match &route_result {
        Ok(()) => BackendControlResponse::ok(response_json.clone()),
        Err(err) => BackendControlResponse::error(-1, err.clone()),
    };
    let _ = request.response_tx.send(response);

    push_plugin_host_event_json(&request.plugin_id, response_json).await;

    let runtime_event = PluginRuntimeEvent::from_payload(
        request.plugin_id.clone(),
        PluginRuntimeKind::ControlResult,
        &payload,
    )
    .unwrap_or_else(|_| PluginRuntimeEvent {
        plugin_id: request.plugin_id.clone(),
        kind: PluginRuntimeKind::ControlResult,
        payload_json: "{}".to_string(),
    });
    emit_runtime_event(router.runtime_hub.as_ref(), runtime_event);

    match route_result {
        Ok(()) => {
            let request_id = parsed_request
                .as_ref()
                .and_then(PluginControlRequest::request_id)
                .cloned();
            let scope = parsed_request
                .as_ref()
                .map(PluginControlRequest::scope)
                .unwrap_or(stellatune_core::ControlScope::Player);
            let command = parsed_request
                .as_ref()
                .map(PluginControlRequest::control_command);
            let wait = parsed_request
                .as_ref()
                .map(control_wait_kind)
                .unwrap_or(ControlWaitKind::Immediate);

            if wait == ControlWaitKind::Immediate {
                emit_control_finished(
                    router.runtime_hub.as_ref(),
                    ControlFinishedArgs {
                        plugin_id: &request.plugin_id,
                        request_id,
                        scope,
                        command,
                        error: None,
                    },
                )
                .await;
            } else {
                pending_finishes.push(PendingControlFinish {
                    plugin_id: request.plugin_id,
                    request_id,
                    scope,
                    command,
                    wait,
                    deadline: Instant::now() + CONTROL_FINISH_TIMEOUT,
                });
            }
        }
        Err(err) => {
            tracing::warn!(
                plugin_id = %request.plugin_id,
                payload = %request.request_json,
                error = %err,
                "failed to route plugin control"
            );
            let request_id = parsed_request
                .as_ref()
                .and_then(PluginControlRequest::request_id)
                .cloned();
            let scope = parsed_request
                .as_ref()
                .map(PluginControlRequest::scope)
                .unwrap_or(stellatune_core::ControlScope::Player);
            let command = parsed_request
                .as_ref()
                .map(PluginControlRequest::control_command);
            emit_control_finished(
                router.runtime_hub.as_ref(),
                ControlFinishedArgs {
                    plugin_id: &request.plugin_id,
                    request_id,
                    scope,
                    command,
                    error: Some(&err),
                },
            )
            .await;
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn plugin_runtime_router() -> &'static std::sync::Arc<PluginRuntimeRouter> {
    static ROUTER: OnceLock<std::sync::Arc<PluginRuntimeRouter>> = OnceLock::new();
    ROUTER.get_or_init(|| {
        let (inbound_tx, mut inbound_rx) = tokio::sync::mpsc::unbounded_channel::<RouterInbound>();
        let router = std::sync::Arc::new(PluginRuntimeRouter {
            engine: std::sync::Mutex::new(None),
            library: std::sync::Mutex::new(None),
            inbound_tx: inbound_tx.clone(),
            player_event_generation: std::sync::atomic::AtomicU64::new(0),
            library_event_generation: std::sync::atomic::AtomicU64::new(0),
            runtime_hub: std::sync::Arc::new(PluginRuntimeEventHub::new()),
        });
        let router_thread = std::sync::Arc::clone(&router);

        global_runtime::spawn(async move {
            let mut control_rx = stellatune_plugins::runtime::handle::shared_runtime_service()
                .subscribe_backend_control_requests()
                .await;
            let mut pending_finishes: Vec<PendingControlFinish> = Vec::new();
            let mut timeout_tick = tokio::time::interval(Duration::from_millis(20));
            loop {
                tokio::select! {
                    Some(request) = control_rx.recv() => {
                        let engine = router_thread.engine.lock().ok().and_then(|g| g.clone());
                        let library = router_thread.library.lock().ok().and_then(|g| g.clone());
                        handle_backend_control_request(
                            router_thread.as_ref(),
                            &mut pending_finishes,
                            request,
                            engine.as_ref(),
                            library.as_ref(),
                        )
                        .await;
                    }
                    Some(message) = inbound_rx.recv() => {
                        match message {
                            RouterInbound::PlayerEvent { generation, event } => {
                                let current = router_thread.player_event_generation.load(std::sync::atomic::Ordering::Relaxed);
                                if generation != current {
                                    continue;
                                }
                                let done = drain_finished_by_player_event(&mut pending_finishes, &event);
                                if should_broadcast_player_event(&event) {
                                    if let Ok(payload_json) = serde_json::to_string(&HostPlayerEventEnvelope {
                                        topic: HostEventTopic::PlayerEvent,
                                        event,
                                    }) {
                                        broadcast_host_event_json(payload_json).await;
                                    }
                                }
                                for done in done {
                                    emit_control_finished(
                                        router_thread.runtime_hub.as_ref(),
                                        ControlFinishedArgs {
                                            plugin_id: &done.plugin_id,
                                            request_id: done.request_id,
                                            scope: done.scope,
                                            command: done.command,
                                            error: None,
                                        },
                                    )
                                    .await;
                                }
                            }
                            RouterInbound::LibraryEvent { generation, event } => {
                                let current = router_thread.library_event_generation.load(std::sync::atomic::Ordering::Relaxed);
                                if generation != current {
                                    continue;
                                }
                                let done = drain_finished_by_library_event(&mut pending_finishes, &event);
                                if should_broadcast_library_event(&event) {
                                    if let Ok(payload_json) = serde_json::to_string(&HostLibraryEventEnvelope {
                                        topic: HostEventTopic::LibraryEvent,
                                        event,
                                    }) {
                                        broadcast_host_event_json(payload_json).await;
                                    }
                                }
                                for done in done {
                                    emit_control_finished(
                                        router_thread.runtime_hub.as_ref(),
                                        ControlFinishedArgs {
                                            plugin_id: &done.plugin_id,
                                            request_id: done.request_id,
                                            scope: done.scope,
                                            command: done.command,
                                            error: None,
                                        },
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                    _ = timeout_tick.tick() => {
                        for timed_out in drain_timed_out_pending(&mut pending_finishes, Instant::now()) {
                            emit_control_finished(
                                router_thread.runtime_hub.as_ref(),
                                ControlFinishedArgs {
                                    plugin_id: &timed_out.plugin_id,
                                    request_id: timed_out.request_id,
                                    scope: timed_out.scope,
                                    command: timed_out.command,
                                    error: Some("control finish timeout"),
                                },
                            )
                            .await;
                        }
                    }
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
    if let Ok(mut slot) = router.engine.lock() {
        *slot = Some(engine);
    }
    let generation = router
        .player_event_generation
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;
    let tx = router.inbound_tx.clone();
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
    if let Ok(mut slot) = router.library.lock() {
        *slot = Some(library);
    }
    let generation = router
        .library_event_generation
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        + 1;
    let tx = router.inbound_tx.clone();
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
