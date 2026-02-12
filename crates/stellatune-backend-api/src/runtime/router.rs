use std::sync::OnceLock;
use std::thread;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Instant;

use stellatune_core::{
    HostEventTopic, HostLibraryEventEnvelope, HostPlayerEventEnvelope, PluginRuntimeEvent,
    PluginRuntimeKind,
};
use stellatune_plugin_protocol::PluginControlRequest;
use stellatune_plugins::PluginRuntimeEvent as PluginRuntimeActorEvent;

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
fn actor_event_to_runtime_event(event: PluginRuntimeActorEvent) -> PluginRuntimeEvent {
    let (plugin_id, payload_json) = match event {
        PluginRuntimeActorEvent::CommandCompleted {
            request_id,
            owner,
            outcome,
        } => {
            let payload = match outcome {
                stellatune_plugins::PluginRuntimeCommandOutcome::SetPluginEnabled {
                    plugin_id,
                    enabled,
                } => (
                    plugin_id.clone(),
                    serde_json::json!({
                        "topic": "host.runtime.actor",
                        "event": "command_completed",
                        "request_id": request_id,
                        "owner": owner,
                        "command": "set_plugin_enabled",
                        "plugin_id": plugin_id,
                        "enabled": enabled
                    }),
                ),
                stellatune_plugins::PluginRuntimeCommandOutcome::ReloadDirFromState {
                    dir,
                    prev_count,
                    loaded_ids,
                    loaded_count,
                    deactivated_count,
                    unloaded_generations,
                    load_errors,
                    fatal_error,
                } => (
                    "host.runtime".to_string(),
                    serde_json::json!({
                        "topic": "host.runtime.actor",
                        "event": "command_completed",
                        "request_id": request_id,
                        "owner": owner,
                        "command": "reload_dir_from_state",
                        "dir": dir,
                        "prev_count": prev_count,
                        "loaded_ids": loaded_ids,
                        "loaded_count": loaded_count,
                        "deactivated_count": deactivated_count,
                        "unloaded_generations": unloaded_generations,
                        "load_errors": load_errors,
                        "fatal_error": fatal_error
                    }),
                ),
                stellatune_plugins::PluginRuntimeCommandOutcome::UnloadPlugin {
                    plugin_id,
                    deactivated,
                    unloaded_generations,
                    remaining_draining_generations,
                    errors,
                } => (
                    plugin_id.clone(),
                    serde_json::json!({
                        "topic": "host.runtime.actor",
                        "event": "command_completed",
                        "request_id": request_id,
                        "owner": owner,
                        "command": "unload_plugin",
                        "plugin_id": plugin_id,
                        "deactivated": deactivated,
                        "unloaded_generations": unloaded_generations,
                        "remaining_draining_generations": remaining_draining_generations,
                        "errors": errors
                    }),
                ),
                stellatune_plugins::PluginRuntimeCommandOutcome::ShutdownAndCleanup {
                    deactivated_count,
                    unloaded_generations,
                    errors,
                } => (
                    "host.runtime".to_string(),
                    serde_json::json!({
                        "topic": "host.runtime.actor",
                        "event": "command_completed",
                        "request_id": request_id,
                        "owner": owner,
                        "command": "shutdown_and_cleanup",
                        "deactivated_count": deactivated_count,
                        "unloaded_generations": unloaded_generations,
                        "errors": errors
                    }),
                ),
            };
            (payload.0, payload.1.to_string())
        }
        PluginRuntimeActorEvent::PluginEnabledChanged { plugin_id, enabled } => (
            plugin_id,
            serde_json::json!({
                "topic": "host.runtime.actor",
                "event": "plugin_enabled_changed",
                "enabled": enabled
            })
            .to_string(),
        ),
        PluginRuntimeActorEvent::PluginsReloaded {
            dir,
            loaded,
            deactivated,
            unloaded_generations,
            errors,
        } => (
            "host.runtime".to_string(),
            serde_json::json!({
                "topic": "host.runtime.actor",
                "event": "plugins_reloaded",
                "dir": dir,
                "loaded": loaded,
                "deactivated": deactivated,
                "unloaded_generations": unloaded_generations,
                "errors": errors
            })
            .to_string(),
        ),
        PluginRuntimeActorEvent::PluginUnloaded {
            plugin_id,
            deactivated,
            unloaded_generations,
            remaining_draining_generations,
        } => (
            plugin_id,
            serde_json::json!({
                "topic": "host.runtime.actor",
                "event": "plugin_unloaded",
                "deactivated": deactivated,
                "unloaded_generations": unloaded_generations,
                "remaining_draining_generations": remaining_draining_generations
            })
            .to_string(),
        ),
        PluginRuntimeActorEvent::RuntimeShutdown {
            deactivated,
            unloaded_generations,
            errors,
        } => (
            "host.runtime".to_string(),
            serde_json::json!({
                "topic": "host.runtime.actor",
                "event": "runtime_shutdown",
                "deactivated": deactivated,
                "unloaded_generations": unloaded_generations,
                "errors": errors
            })
            .to_string(),
        ),
    };

    PluginRuntimeEvent {
        plugin_id,
        kind: PluginRuntimeKind::Notify,
        payload_json,
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn drain_actor_events(
    rx: &crossbeam_channel::Receiver<PluginRuntimeActorEvent>,
    max: usize,
) -> Vec<PluginRuntimeActorEvent> {
    let mut out = Vec::new();
    for _ in 0..max {
        match rx.try_recv() {
            Ok(event) => out.push(event),
            Err(crossbeam_channel::TryRecvError::Empty) => break,
            Err(crossbeam_channel::TryRecvError::Disconnected) => break,
        }
    }
    out
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
        let actor_events = stellatune_plugins::shared_runtime_service().subscribe_runtime_events();
        if let Err(e) = thread::Builder::new()
            .name("stellatune-plugin-runtime-router".to_string())
            .spawn(move || {
                let mut pending_finishes: Vec<PendingControlFinish> = Vec::new();
                loop {
                    let engine = router_thread.engine.lock().ok().and_then(|g| g.clone());
                    let library = router_thread.library.lock().ok().and_then(|g| g.clone());

                    let runtime_events = stellatune_plugins::drain_shared_runtime_events(128);

                    for event in runtime_events {
                        emit_runtime_event(router_thread.runtime_hub.as_ref(), event.clone());
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

                            push_plugin_host_event_json(&event.plugin_id, response_json.clone());

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

                            emit_runtime_event(router_thread.runtime_hub.as_ref(), runtime_event);

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
                                            router_thread.runtime_hub.as_ref(),
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
                                        router_thread.runtime_hub.as_ref(),
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

                    for actor_event in drain_actor_events(&actor_events, 128) {
                        let runtime_event = actor_event_to_runtime_event(actor_event);
                        emit_runtime_event(router_thread.runtime_hub.as_ref(), runtime_event);
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
