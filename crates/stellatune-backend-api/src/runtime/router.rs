use std::sync::OnceLock;
use std::thread;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Instant;

use stellatune_core::{
    HostEventTopic, HostLibraryEventEnvelope, HostPlayerEventEnvelope, PluginRuntimeEvent,
    PluginRuntimeKind,
};
use stellatune_plugin_protocol::PluginControlRequest;

use super::bus::{
    ControlFinishedArgs, broadcast_host_event_json, build_control_result_payload,
    drain_finished_by_library_event, drain_finished_by_player_event, drain_router_receiver,
    drain_timed_out_pending, emit_control_finished, emit_runtime_event,
    push_plugin_host_event_json,
};
use super::control::{control_wait_kind, route_plugin_control_request};
use super::shared_plugins;
use super::types::{
    CONTROL_FINISH_TIMEOUT, ControlWaitKind, PendingControlFinish, PluginRuntimeEventHub,
    PluginRuntimeRouter,
};

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
        let plugins = shared_plugins();
        thread::Builder::new()
            .name("stellatune-plugin-runtime-router".to_string())
            .spawn(move || {
                let mut pending_finishes: Vec<PendingControlFinish> = Vec::new();
                loop {
                    let engine = router_thread.engine.lock().ok().and_then(|g| g.clone());
                    let library = router_thread.library.lock().ok().and_then(|g| g.clone());

                    let runtime_events = match plugins.lock() {
                        Ok(pm) => pm.drain_runtime_events(128),
                        Err(_) => {
                            thread::sleep(std::time::Duration::from_millis(20));
                            continue;
                        }
                    };

                    for event in runtime_events {
                        emit_runtime_event(
                            router_thread.runtime_hub.as_ref(),
                            engine.as_ref(),
                            event.clone(),
                        );
                        if event.kind == PluginRuntimeKind::Control {
                            let (request, route_result) =
                                match event.payload::<PluginControlRequest>() {
                                    Ok(request) => {
                                        let result = route_plugin_control_request(
                                            &request,
                                            engine.as_ref(),
                                            library.as_ref(),
                                        );
                                        (Some(request), result)
                                    }
                                    Err(err) => (None, Err(format!("invalid json: {err}"))),
                                };

                            let payload = build_control_result_payload(
                                request.as_ref(),
                                route_result.as_ref().err().map(String::as_str),
                            );
                            let response_json = serde_json::to_string(&payload)
                                .unwrap_or_else(|_| "{}".to_string());

                            push_plugin_host_event_json(
                                &plugins,
                                &event.plugin_id,
                                response_json.clone(),
                            );

                            let runtime_event = PluginRuntimeEvent::from_payload(
                                event.plugin_id.clone(),
                                PluginRuntimeKind::ControlResult,
                                &payload,
                            )
                            .unwrap_or_else(|_| PluginRuntimeEvent {
                                plugin_id: event.plugin_id.clone(),
                                kind: PluginRuntimeKind::ControlResult,
                                payload_json: "{}".to_string(),
                            });

                            emit_runtime_event(
                                router_thread.runtime_hub.as_ref(),
                                engine.as_ref(),
                                runtime_event,
                            );

                            match route_result {
                                Ok(()) => {
                                    let request_id = request
                                        .as_ref()
                                        .and_then(PluginControlRequest::request_id)
                                        .cloned();
                                    let scope = request
                                        .as_ref()
                                        .map(PluginControlRequest::scope)
                                        .unwrap_or(stellatune_core::ControlScope::Player);
                                    let command =
                                        request.as_ref().map(PluginControlRequest::control_command);
                                    let wait = request
                                        .as_ref()
                                        .map(control_wait_kind)
                                        .unwrap_or(ControlWaitKind::Immediate);

                                    if wait == ControlWaitKind::Immediate {
                                        emit_control_finished(
                                            &plugins,
                                            router_thread.runtime_hub.as_ref(),
                                            engine.as_ref(),
                                            ControlFinishedArgs {
                                                plugin_id: &event.plugin_id,
                                                request_id,
                                                scope,
                                                command,
                                                error: None,
                                            },
                                        );
                                    } else {
                                        pending_finishes.push(PendingControlFinish {
                                            plugin_id: event.plugin_id.clone(),
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
                                        plugin_id = %event.plugin_id,
                                        payload = %event.payload_json,
                                        error = %err,
                                        "failed to route plugin control"
                                    );
                                    let request_id = request
                                        .as_ref()
                                        .and_then(PluginControlRequest::request_id)
                                        .cloned();
                                    let scope = request
                                        .as_ref()
                                        .map(PluginControlRequest::scope)
                                        .unwrap_or(stellatune_core::ControlScope::Player);
                                    let command =
                                        request.as_ref().map(PluginControlRequest::control_command);
                                    emit_control_finished(
                                        &plugins,
                                        router_thread.runtime_hub.as_ref(),
                                        engine.as_ref(),
                                        ControlFinishedArgs {
                                            plugin_id: &event.plugin_id,
                                            request_id,
                                            scope,
                                            command,
                                            error: Some(&err),
                                        },
                                    );
                                }
                            }
                        }
                    }

                    for event in drain_router_receiver(&router_thread.player_events, 128) {
                        let done = drain_finished_by_player_event(&mut pending_finishes, &event);
                        if let Ok(payload_json) = serde_json::to_string(&HostPlayerEventEnvelope {
                            topic: HostEventTopic::PlayerEvent,
                            event,
                        }) {
                            broadcast_host_event_json(&plugins, payload_json);
                        }
                        for done in done {
                            emit_control_finished(
                                &plugins,
                                router_thread.runtime_hub.as_ref(),
                                engine.as_ref(),
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
                            broadcast_host_event_json(&plugins, payload_json);
                        }
                        for done in done {
                            emit_control_finished(
                                &plugins,
                                router_thread.runtime_hub.as_ref(),
                                engine.as_ref(),
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
                            &plugins,
                            router_thread.runtime_hub.as_ref(),
                            engine.as_ref(),
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
            .expect("failed to spawn stellatune-plugin-runtime-router");
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
