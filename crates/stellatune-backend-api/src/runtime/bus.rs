#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::time::Instant;

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use crossbeam_channel::{Receiver as CbReceiver, TryRecvError};

use stellatune_core::{
    ControlCommand, ControlScope, Event, HostControlFinishedPayload, HostControlResultPayload,
    HostEventTopic, LibraryEvent, PluginRuntimeEvent, PluginRuntimeKind,
};
use stellatune_plugin_protocol::{PluginControlRequest, RequestId};

use super::control::{
    control_scope_from_request, is_wait_satisfied_by_library, is_wait_satisfied_by_player,
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
pub(super) fn push_plugin_host_event_json(plugin_id: &str, event_json: String) {
    stellatune_plugins::push_shared_host_event_json(plugin_id, &event_json);
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn broadcast_host_event_json(event_json: String) {
    stellatune_plugins::broadcast_shared_host_event_json(&event_json);
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn build_control_result_payload(
    request: Option<&PluginControlRequest>,
    error: Option<&str>,
) -> HostControlResultPayload {
    let scope = control_scope_from_request(request);
    HostControlResultPayload {
        topic: HostEventTopic::HostControlResult,
        request_id: request.and_then(PluginControlRequest::request_id).cloned(),
        scope,
        command: request.map(PluginControlRequest::control_command),
        ok: error.is_none(),
        error: error.map(|v| v.to_string()),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
#[cfg(test)]
pub(super) fn build_control_result_event_json(
    request: Option<&PluginControlRequest>,
    error: Option<&str>,
) -> String {
    serde_json::to_string(&build_control_result_payload(request, error))
        .unwrap_or_else(|_| "{}".to_string())
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn build_control_finished_payload(
    request_id: Option<RequestId>,
    scope: ControlScope,
    command: Option<ControlCommand>,
    error: Option<&str>,
) -> HostControlFinishedPayload {
    HostControlFinishedPayload {
        topic: HostEventTopic::HostControlFinished,
        request_id,
        scope,
        command,
        ok: error.is_none(),
        error: error.map(|v| v.to_string()),
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
#[cfg(test)]
pub(super) fn build_control_finished_event_json(
    request_id: Option<RequestId>,
    scope: ControlScope,
    command: Option<ControlCommand>,
    error: Option<&str>,
) -> String {
    serde_json::to_string(&build_control_finished_payload(
        request_id, scope, command, error,
    ))
    .unwrap_or_else(|_| "{}".to_string())
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn emit_runtime_event(hub: &PluginRuntimeEventHub, event: PluginRuntimeEvent) {
    hub.emit(event);
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) struct ControlFinishedArgs<'a> {
    pub(super) plugin_id: &'a str,
    pub(super) request_id: Option<RequestId>,
    pub(super) scope: ControlScope,
    pub(super) command: Option<ControlCommand>,
    pub(super) error: Option<&'a str>,
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(super) fn emit_control_finished(hub: &PluginRuntimeEventHub, args: ControlFinishedArgs<'_>) {
    let payload =
        build_control_finished_payload(args.request_id, args.scope, args.command, args.error);
    let payload_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    push_plugin_host_event_json(args.plugin_id, payload_json);
    let event = PluginRuntimeEvent::from_payload(
        args.plugin_id.to_string(),
        PluginRuntimeKind::ControlFinished,
        &payload,
    )
    .unwrap_or_else(|_| PluginRuntimeEvent {
        plugin_id: args.plugin_id.to_string(),
        kind: PluginRuntimeKind::ControlFinished,
        payload_json: "{}".to_string(),
    });
    emit_runtime_event(hub, event);
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
