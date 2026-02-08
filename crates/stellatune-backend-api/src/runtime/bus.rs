#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Instant;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use crossbeam_channel::{Receiver as CbReceiver, TryRecvError};

use stellatune_core::{
    ControlCommand, ControlScope, Event, HostControlFinishedPayload, HostControlResultPayload,
    HostEventTopic, LibraryEvent, PluginRuntimeEvent, PluginRuntimeKind,
};

use super::SharedPlugins;
use super::control::{
    control_command_from_root, control_scope_from_root, is_wait_satisfied_by_library,
    is_wait_satisfied_by_player,
};
use super::types::{PendingControlFinish, PluginRuntimeEventHub};

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn drain_router_receiver<T>(
    slot: &std::sync::Mutex<Option<CbReceiver<T>>>,
    max: usize,
) -> Vec<T> {
    let mut out = Vec::new();
    let mut disconnected = false;

    if let Ok(mut guard) = slot.lock() {
        let Some(rx) = guard.as_ref() else {
            return out;
        };
        for _ in 0..max {
            match rx.try_recv() {
                Ok(item) => out.push(item),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected {
            *guard = None;
        }
    }

    out
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn push_plugin_host_event_json(
    plugins: &SharedPlugins,
    plugin_id: &str,
    event_json: String,
) {
    if let Ok(pm) = plugins.lock()
        && let Err(err) = pm.push_host_event_json(plugin_id, &event_json)
    {
        tracing::warn!(
            plugin_id = plugin_id,
            error = %err,
            "failed to push host event to plugin"
        );
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn broadcast_host_event_json(plugins: &SharedPlugins, event_json: String) {
    if let Ok(pm) = plugins.lock() {
        pm.broadcast_host_event_json(&event_json);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn build_control_result_event_json(
    root: Option<&serde_json::Value>,
    error: Option<&str>,
) -> String {
    let scope = control_scope_from_root(root);
    let payload = HostControlResultPayload {
        topic: HostEventTopic::HostControlResult,
        request_id: root.and_then(|v| v.get("request_id")).cloned(),
        scope,
        command: control_command_from_root(root, scope),
        ok: error.is_none(),
        error: error.map(|v| v.to_string()),
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn build_control_finished_event_json(
    request_id: Option<serde_json::Value>,
    scope: ControlScope,
    command: Option<ControlCommand>,
    error: Option<&str>,
) -> String {
    let payload = HostControlFinishedPayload {
        topic: HostEventTopic::HostControlFinished,
        request_id,
        scope,
        command,
        ok: error.is_none(),
        error: error.map(|v| v.to_string()),
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn emit_runtime_event(
    hub: &PluginRuntimeEventHub,
    engine: Option<&stellatune_audio::EngineHandle>,
    event: PluginRuntimeEvent,
) {
    hub.emit(event.clone());
    if let Some(engine) = engine {
        engine.emit_plugin_runtime_event(event);
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) struct ControlFinishedArgs<'a> {
    pub(super) plugin_id: &'a str,
    pub(super) request_id: Option<serde_json::Value>,
    pub(super) scope: ControlScope,
    pub(super) command: Option<ControlCommand>,
    pub(super) error: Option<&'a str>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn emit_control_finished(
    plugins: &SharedPlugins,
    hub: &PluginRuntimeEventHub,
    engine: Option<&stellatune_audio::EngineHandle>,
    args: ControlFinishedArgs<'_>,
) {
    let payload_json =
        build_control_finished_event_json(args.request_id, args.scope, args.command, args.error);
    push_plugin_host_event_json(plugins, args.plugin_id, payload_json.clone());
    emit_runtime_event(
        hub,
        engine,
        PluginRuntimeEvent {
            plugin_id: args.plugin_id.to_string(),
            kind: PluginRuntimeKind::ControlFinished,
            payload_json,
        },
    );
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn drain_finished_by_player_event(
    pending: &mut Vec<PendingControlFinish>,
    event: &Event,
) -> Vec<PendingControlFinish> {
    let mut done = Vec::new();
    let mut i = 0usize;
    while i < pending.len() {
        if is_wait_satisfied_by_player(pending[i].wait, event) {
            done.push(pending.swap_remove(i));
        } else {
            i += 1;
        }
    }
    done
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn drain_finished_by_library_event(
    pending: &mut Vec<PendingControlFinish>,
    event: &LibraryEvent,
) -> Vec<PendingControlFinish> {
    let mut done = Vec::new();
    let mut i = 0usize;
    while i < pending.len() {
        if is_wait_satisfied_by_library(pending[i].wait, event) {
            done.push(pending.swap_remove(i));
        } else {
            i += 1;
        }
    }
    done
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn drain_timed_out_pending(
    pending: &mut Vec<PendingControlFinish>,
    now: Instant,
) -> Vec<PendingControlFinish> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < pending.len() {
        if pending[i].deadline <= now {
            out.push(pending.swap_remove(i));
        } else {
            i += 1;
        }
    }
    out
}
