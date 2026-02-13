use std::sync::OnceLock;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::thread;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Instant;

use stellatune_core::{
    HostEventTopic, HostLibraryEventEnvelope, HostPlayerEventEnvelope, PluginRuntimeEvent,
    PluginRuntimeKind,
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use crossbeam_channel::TryRecvError;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugin_protocol::PluginControlRequest;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use stellatune_plugins::runtime::backend_control::{BackendControlRequest, BackendControlResponse};

use super::bus::{
    ControlFinishedArgs, broadcast_host_event_json, build_control_result_payload,
    drain_finished_by_library_event, drain_finished_by_player_event, drain_router_receiver,
    drain_timed_out_pending, emit_control_finished, emit_runtime_event,
    push_plugin_host_event_json,
};
use super::control::{control_wait_kind, route_plugin_control_request};
use super::types::{
    CONTROL_FINISH_TIMEOUT, ControlWaitKind, PendingControlFinish, PluginRuntimeEventHub,
    PluginRuntimeRouter,
};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn drain_backend_control_requests(
    rx: &crossbeam_channel::Receiver<BackendControlRequest>,
    max: usize,
) -> Vec<BackendControlRequest> {
    let mut out = Vec::new();
    for _ in 0..max {
        match rx.try_recv() {
            Ok(request) => out.push(request),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
    out
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn handle_backend_control_request(
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
        Some(parsed) => route_plugin_control_request(parsed, engine, library),
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

    push_plugin_host_event_json(&request.plugin_id, response_json);

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
                );
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
            );
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn plugin_runtime_router() -> &'static std::sync::Arc<PluginRuntimeRouter> {
    static ROUTER: OnceLock<std::sync::Arc<PluginRuntimeRouter>> = OnceLock::new();
    ROUTER.get_or_init(|| {
        let router = std::sync::Arc::new(PluginRuntimeRouter {
            engine: std::sync::Mutex::new(None),
            library: std::sync::Mutex::new(None),
            player_events: std::sync::Mutex::new(None),
            library_events: std::sync::Mutex::new(None),
            runtime_hub: std::sync::Arc::new(PluginRuntimeEventHub::new()),
        });
        let router_thread = std::sync::Arc::clone(&router);
        let control_rx = stellatune_plugins::runtime::handle::shared_runtime_service()
            .subscribe_backend_control_requests();

        if let Err(e) = thread::Builder::new()
            .name("stellatune-plugin-runtime-router".to_string())
            .spawn(move || {
                let mut pending_finishes: Vec<PendingControlFinish> = Vec::new();
                loop {
                    let engine = router_thread.engine.lock().ok().and_then(|g| g.clone());
                    let library = router_thread.library.lock().ok().and_then(|g| g.clone());

                    for request in drain_backend_control_requests(&control_rx, 128) {
                        handle_backend_control_request(
                            router_thread.as_ref(),
                            &mut pending_finishes,
                            request,
                            engine.as_ref(),
                            library.as_ref(),
                        );
                    }

                    for event in drain_router_receiver(&router_thread.player_events, 128) {
                        let done = drain_finished_by_player_event(&mut pending_finishes, &event);
                        if let Ok(payload_json) = serde_json::to_string(&HostPlayerEventEnvelope {
                            topic: HostEventTopic::PlayerEvent,
                            event,
                        }) {
                            broadcast_host_event_json(payload_json);
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
                            );
                        }
                    }

                    for event in drain_router_receiver(&router_thread.library_events, 128) {
                        let done = drain_finished_by_library_event(&mut pending_finishes, &event);
                        if let Ok(payload_json) = serde_json::to_string(&HostLibraryEventEnvelope {
                            topic: HostEventTopic::LibraryEvent,
                            event,
                        }) {
                            broadcast_host_event_json(payload_json);
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
                            );
                        }
                    }

                    for timed_out in drain_timed_out_pending(&mut pending_finishes, Instant::now())
                    {
                        emit_control_finished(
                            router_thread.runtime_hub.as_ref(),
                            ControlFinishedArgs {
                                plugin_id: &timed_out.plugin_id,
                                request_id: timed_out.request_id,
                                scope: timed_out.scope,
                                command: timed_out.command,
                                error: Some("control finish timeout"),
                            },
                        );
                    }

                    thread::sleep(std::time::Duration::from_millis(10));
                }
            })
        {
            tracing::error!("failed to spawn stellatune-plugin-runtime-router: {e}");
        }

        router
    })
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn register_plugin_runtime_engine(engine: stellatune_audio::EngineHandle) {
    let router = plugin_runtime_router();
    let player_rx = engine.subscribe_events();
    if let Ok(mut slot) = router.engine.lock() {
        *slot = Some(engine);
    }
    if let Ok(mut slot) = router.player_events.lock() {
        *slot = Some(player_rx);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn register_plugin_runtime_library(library: stellatune_library::LibraryHandle) {
    let router = plugin_runtime_router();
    let library_rx = library.subscribe_events();
    if let Ok(mut slot) = router.library.lock() {
        *slot = Some(library);
    }
    if let Ok(mut slot) = router.library_events.lock() {
        *slot = Some(library_rx);
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn register_plugin_runtime_engine(_engine: stellatune_audio::EngineHandle) {}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn register_plugin_runtime_library(_library: stellatune_library::LibraryHandle) {}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub fn subscribe_plugin_runtime_events_global() -> crossbeam_channel::Receiver<PluginRuntimeEvent> {
    plugin_runtime_router().runtime_hub.subscribe()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub fn subscribe_plugin_runtime_events_global() -> crossbeam_channel::Receiver<PluginRuntimeEvent> {
    let (_tx, rx) = crossbeam_channel::unbounded();
    rx
}
