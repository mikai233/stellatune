use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::RecvTimeoutError;
use stellatune_plugin_api::StHostVTable;
use stellatune_plugin_api::{
    ST_ERR_INTERNAL, ST_ERR_INVALID_ARG, ST_ERR_UNSUPPORTED, StAsyncOpState, StJsonOpRef,
    StJsonOpVTable, StOpNotifier, StStatus, StStr,
};
use tokio::sync::mpsc;

use crate::runtime::backend_control::{BackendControlRequest, BackendControlResponse};

const HOST_TO_PLUGIN_QUEUE_CAP: usize = 512;
const CONTROL_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlDispatchError {
    NoHandler,
    Timeout,
    ResponseDropped,
}

#[derive(Debug, Default)]
struct PluginEventBusState {
    host_to_plugin: HashMap<String, VecDeque<String>>,
    plugin_ref_counts: HashMap<String, usize>,
    control_request_senders: Vec<mpsc::UnboundedSender<BackendControlRequest>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PluginEventBus {
    inner: Arc<Mutex<PluginEventBusState>>,
    per_plugin_queue_cap: usize,
}

impl PluginEventBus {
    pub(crate) fn new(per_plugin_queue_cap: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PluginEventBusState::default())),
            per_plugin_queue_cap,
        }
    }

    pub(crate) fn acquire_plugin(&self, plugin_id: &str) {
        if let Ok(mut state) = self.inner.lock() {
            let count = state
                .plugin_ref_counts
                .entry(plugin_id.to_string())
                .or_insert(0);
            *count = count.saturating_add(1);
            state
                .host_to_plugin
                .entry(plugin_id.to_string())
                .or_insert_with(VecDeque::new);
        }
    }

    pub(crate) fn release_plugin(&self, plugin_id: &str) {
        if let Ok(mut state) = self.inner.lock() {
            let should_drop = match state.plugin_ref_counts.get_mut(plugin_id) {
                Some(count) => {
                    *count = count.saturating_sub(1);
                    *count == 0
                }
                None => false,
            };
            if should_drop {
                state.plugin_ref_counts.remove(plugin_id);
                state.host_to_plugin.remove(plugin_id);
            }
        }
    }

    pub(crate) fn registered_plugin_ids(&self) -> Vec<String> {
        let Ok(state) = self.inner.lock() else {
            return Vec::new();
        };
        state.host_to_plugin.keys().cloned().collect()
    }

    pub(crate) fn push_host_event(&self, plugin_id: &str, event_json: String) {
        if let Ok(mut state) = self.inner.lock() {
            let Some(queue) = state.host_to_plugin.get_mut(plugin_id) else {
                return;
            };
            if queue.len() >= self.per_plugin_queue_cap {
                queue.pop_front();
            }
            queue.push_back(event_json);
        }
    }

    pub(crate) fn poll_host_event(&self, plugin_id: &str) -> Option<String> {
        let mut state = self.inner.lock().ok()?;
        state
            .host_to_plugin
            .get_mut(plugin_id)
            .and_then(|queue| queue.pop_front())
    }

    pub(crate) fn register_control_request_sender(
        &self,
        sender: mpsc::UnboundedSender<BackendControlRequest>,
    ) {
        if let Ok(mut state) = self.inner.lock() {
            state.control_request_senders.push(sender);
        }
    }

    pub(crate) fn subscribe_control_requests(
        &self,
    ) -> mpsc::UnboundedReceiver<BackendControlRequest> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.register_control_request_sender(tx);
        rx
    }

    fn send_control_request(
        &self,
        plugin_id: &str,
        request_json: String,
    ) -> Result<BackendControlResponse, ControlDispatchError> {
        let (response_tx, response_rx) = crossbeam_channel::bounded(1);
        let request = BackendControlRequest {
            plugin_id: plugin_id.to_string(),
            request_json,
            response_tx,
        };

        let mut dispatched = false;
        if let Ok(mut state) = self.inner.lock() {
            let mut dead_indices = Vec::new();
            for (idx, sender) in state.control_request_senders.iter().enumerate() {
                match sender.send(request.clone()) {
                    Ok(()) => {
                        dispatched = true;
                        break;
                    }
                    Err(_) => dead_indices.push(idx),
                }
            }
            for idx in dead_indices.into_iter().rev() {
                state.control_request_senders.remove(idx);
            }
        }
        if !dispatched {
            return Err(ControlDispatchError::NoHandler);
        }

        match response_rx.recv_timeout(CONTROL_REQUEST_TIMEOUT) {
            Ok(response) => Ok(response),
            Err(RecvTimeoutError::Timeout) => Err(ControlDispatchError::Timeout),
            Err(RecvTimeoutError::Disconnected) => Err(ControlDispatchError::ResponseDropped),
        }
    }
}

pub(crate) fn new_runtime_event_bus() -> PluginEventBus {
    PluginEventBus::new(HOST_TO_PLUGIN_QUEUE_CAP)
}

#[derive(Debug, Clone)]
pub(crate) struct PluginHostCtx {
    plugin_id: String,
    runtime_root_utf8: Box<[u8]>,
    event_bus: PluginEventBus,
}

impl PluginHostCtx {
    fn new(plugin_id: String, runtime_root_utf8: Box<[u8]>, event_bus: PluginEventBus) -> Self {
        Self {
            plugin_id,
            runtime_root_utf8,
            event_bus,
        }
    }
}

impl Drop for PluginHostCtx {
    fn drop(&mut self) {
        self.event_bus.release_plugin(&self.plugin_id);
    }
}

extern "C" fn plugin_host_runtime_root(user_data: *mut core::ffi::c_void) -> StStr {
    if user_data.is_null() {
        return StStr::empty();
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    if ctx.runtime_root_utf8.is_empty() {
        return StStr::empty();
    }
    StStr {
        ptr: ctx.runtime_root_utf8.as_ptr(),
        len: ctx.runtime_root_utf8.len(),
    }
}

extern "C" fn plugin_host_poll_event_json(
    user_data: *mut core::ffi::c_void,
    out_op: *mut StJsonOpRef,
) -> StStatus {
    if user_data.is_null() || out_op.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let op = HostJsonOp::ready(ctx.event_bus.poll_host_event(&ctx.plugin_id));
    unsafe {
        *out_op = StJsonOpRef {
            handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
            vtable: &HOST_JSON_OP_VTABLE as *const StJsonOpVTable,
            reserved0: 0,
            reserved1: 0,
        };
    }
    StStatus::ok()
}

extern "C" fn plugin_host_send_control_json(
    user_data: *mut core::ffi::c_void,
    request_json_utf8: StStr,
    out_op: *mut StJsonOpRef,
) -> StStatus {
    if user_data.is_null() || out_op.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let payload = unsafe { crate::util::ststr_to_string_lossy(request_json_utf8) };

    let response = match ctx.event_bus.send_control_request(&ctx.plugin_id, payload) {
        Ok(response) => response,
        Err(ControlDispatchError::NoHandler) => {
            return status_error(
                ST_ERR_UNSUPPORTED,
                "backend control handler is not registered",
            );
        }
        Err(ControlDispatchError::Timeout) => {
            return status_error(ST_ERR_INTERNAL, "backend control handler timed out");
        }
        Err(ControlDispatchError::ResponseDropped) => {
            return status_error(ST_ERR_INTERNAL, "backend control handler dropped response");
        }
    };
    if response.status_code != 0 {
        return status_error(
            response.status_code,
            response
                .error_message
                .as_deref()
                .unwrap_or("backend control request failed"),
        );
    }

    let op = HostJsonOp::ready(Some(response.response_json));
    unsafe {
        *out_op = StJsonOpRef {
            handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
            vtable: &HOST_JSON_OP_VTABLE as *const StJsonOpVTable,
            reserved0: 0,
            reserved1: 0,
        };
    }
    StStatus::ok()
}

#[derive(Debug)]
struct HostJsonOpInner {
    state: StAsyncOpState,
    payload: Option<String>,
    taken: bool,
    notifier: Option<StOpNotifier>,
}

#[derive(Debug)]
struct HostJsonOp {
    inner: Mutex<HostJsonOpInner>,
}

impl HostJsonOp {
    fn ready(payload: Option<String>) -> Self {
        Self {
            inner: Mutex::new(HostJsonOpInner {
                state: StAsyncOpState::Ready,
                payload,
                taken: false,
                notifier: None,
            }),
        }
    }

    fn notify(notifier: Option<StOpNotifier>) {
        let Some(notifier) = notifier else {
            return;
        };
        let Some(cb) = notifier.notify else {
            return;
        };
        cb(notifier.user_data);
    }
}

extern "C" fn host_json_op_poll(
    handle: *mut core::ffi::c_void,
    out_state: *mut StAsyncOpState,
) -> StStatus {
    if handle.is_null() || out_state.is_null() {
        return status_error(ST_ERR_INVALID_ARG, "null handle/out_state");
    }
    let op = unsafe { &*(handle as *mut HostJsonOp) };
    let Ok(inner) = op.inner.lock() else {
        return status_error(ST_ERR_INTERNAL, "host json op lock poisoned");
    };
    unsafe {
        *out_state = inner.state;
    }
    StStatus::ok()
}

extern "C" fn host_json_op_wait(
    handle: *mut core::ffi::c_void,
    _timeout_ms: u32,
    out_state: *mut StAsyncOpState,
) -> StStatus {
    host_json_op_poll(handle, out_state)
}

extern "C" fn host_json_op_cancel(handle: *mut core::ffi::c_void) -> StStatus {
    if handle.is_null() {
        return status_error(ST_ERR_INVALID_ARG, "null handle");
    }
    let op = unsafe { &*(handle as *mut HostJsonOp) };
    let notifier = {
        let Ok(mut inner) = op.inner.lock() else {
            return status_error(ST_ERR_INTERNAL, "host json op lock poisoned");
        };
        inner.state = StAsyncOpState::Cancelled;
        inner.notifier
    };
    HostJsonOp::notify(notifier);
    StStatus::ok()
}

extern "C" fn host_json_op_set_notifier(
    handle: *mut core::ffi::c_void,
    notifier: StOpNotifier,
) -> StStatus {
    if handle.is_null() {
        return status_error(ST_ERR_INVALID_ARG, "null handle");
    }
    let op = unsafe { &*(handle as *mut HostJsonOp) };
    let (should_notify, notifier_copy) = {
        let Ok(mut inner) = op.inner.lock() else {
            return status_error(ST_ERR_INTERNAL, "host json op lock poisoned");
        };
        inner.notifier = Some(notifier);
        (inner.state != StAsyncOpState::Pending, inner.notifier)
    };
    if should_notify {
        HostJsonOp::notify(notifier_copy);
    }
    StStatus::ok()
}

extern "C" fn host_json_op_take_json_utf8(
    handle: *mut core::ffi::c_void,
    out_json_utf8: *mut StStr,
) -> StStatus {
    if handle.is_null() || out_json_utf8.is_null() {
        return status_error(ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
    }
    let op = unsafe { &*(handle as *mut HostJsonOp) };
    let payload = {
        let Ok(mut inner) = op.inner.lock() else {
            return status_error(ST_ERR_INTERNAL, "host json op lock poisoned");
        };
        match inner.state {
            StAsyncOpState::Pending => {
                return status_error(ST_ERR_INTERNAL, "host json op still pending");
            }
            StAsyncOpState::Cancelled => {
                return status_error(ST_ERR_INTERNAL, "host json op cancelled");
            }
            StAsyncOpState::Failed => {
                return status_error(ST_ERR_INTERNAL, "host json op failed");
            }
            StAsyncOpState::Ready => {}
        }
        if inner.taken {
            return status_error(ST_ERR_INVALID_ARG, "host json op result already taken");
        }
        inner.taken = true;
        inner.payload.take()
    };
    let out = payload
        .as_deref()
        .map(alloc_host_owned_ststr)
        .unwrap_or_else(StStr::empty);
    unsafe {
        *out_json_utf8 = out;
    }
    StStatus::ok()
}

extern "C" fn host_json_op_destroy(handle: *mut core::ffi::c_void) {
    if handle.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle as *mut HostJsonOp));
    }
}

static HOST_JSON_OP_VTABLE: StJsonOpVTable = StJsonOpVTable {
    poll: host_json_op_poll,
    wait: host_json_op_wait,
    cancel: host_json_op_cancel,
    set_notifier: host_json_op_set_notifier,
    take_json_utf8: host_json_op_take_json_utf8,
    destroy: host_json_op_destroy,
};

extern "C" fn plugin_host_free_str(_user_data: *mut core::ffi::c_void, s: StStr) {
    free_host_owned_ststr(s);
}

fn alloc_host_owned_ststr(text: &str) -> StStr {
    if text.is_empty() {
        return StStr::empty();
    }
    let boxed = text.as_bytes().to_vec().into_boxed_slice();
    let out = StStr {
        ptr: boxed.as_ptr(),
        len: boxed.len(),
    };
    std::mem::forget(boxed);
    out
}

fn status_error(code: i32, message: &str) -> StStatus {
    let message = if message.is_empty() {
        StStr::empty()
    } else {
        alloc_host_owned_ststr(message)
    };
    StStatus { code, message }
}

fn free_host_owned_ststr(s: StStr) {
    if s.ptr.is_null() || s.len == 0 {
        return;
    }
    unsafe {
        let raw = std::ptr::slice_from_raw_parts_mut(s.ptr as *mut u8, s.len);
        drop(Box::from_raw(raw));
    }
}

pub(crate) fn build_plugin_host_vtable(
    base_host: &StHostVTable,
    plugin_id: &str,
    runtime_root: &Path,
    event_bus: PluginEventBus,
) -> (Box<StHostVTable>, Box<PluginHostCtx>) {
    event_bus.acquire_plugin(plugin_id);

    let mut host_ctx = Box::new(PluginHostCtx::new(
        plugin_id.to_string(),
        runtime_root
            .to_string_lossy()
            .into_owned()
            .into_bytes()
            .into_boxed_slice(),
        event_bus,
    ));
    let mut host_vtable = Box::new(*base_host);
    host_vtable.user_data = (&mut *host_ctx) as *mut PluginHostCtx as *mut core::ffi::c_void;
    host_vtable.get_runtime_root_utf8 = Some(plugin_host_runtime_root);
    host_vtable.emit_event_json_utf8 = None;
    host_vtable.begin_poll_host_event_json_utf8 = Some(plugin_host_poll_event_json);
    host_vtable.begin_send_control_json_utf8 = Some(plugin_host_send_control_json);
    host_vtable.free_host_str_utf8 = Some(plugin_host_free_str);
    (host_vtable, host_ctx)
}

#[cfg(test)]
#[path = "tests/events_tests.rs"]
mod tests;
