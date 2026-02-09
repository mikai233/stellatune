use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use stellatune_core::{PluginRuntimeEvent, PluginRuntimeKind};
use stellatune_plugin_api::StHostVTable;
use stellatune_plugin_api::{ST_ERR_INVALID_ARG, StStatus, StStr};

const HOST_TO_PLUGIN_QUEUE_CAP: usize = 512;
const PLUGIN_TO_HOST_QUEUE_CAP: usize = 2048;

#[derive(Debug, Default)]
struct PluginEventBusState {
    host_to_plugin: HashMap<String, VecDeque<String>>,
    plugin_to_host: VecDeque<PluginRuntimeEvent>,
    plugin_ref_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct PluginEventBus {
    inner: Arc<Mutex<PluginEventBusState>>,
    per_plugin_queue_cap: usize,
    outbound_queue_cap: usize,
}

impl PluginEventBus {
    fn new(per_plugin_queue_cap: usize, outbound_queue_cap: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PluginEventBusState::default())),
            per_plugin_queue_cap,
            outbound_queue_cap,
        }
    }

    fn acquire_plugin(&self, plugin_id: &str) {
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

    fn release_plugin(&self, plugin_id: &str) {
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
                state.plugin_to_host.retain(|ev| ev.plugin_id != plugin_id);
            }
        }
    }

    fn registered_plugin_ids(&self) -> Vec<String> {
        let Ok(state) = self.inner.lock() else {
            return Vec::new();
        };
        state.host_to_plugin.keys().cloned().collect()
    }

    fn push_host_event(&self, plugin_id: &str, event_json: String) {
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

    fn poll_host_event(&self, plugin_id: &str) -> Option<String> {
        let mut state = self.inner.lock().ok()?;
        state
            .host_to_plugin
            .get_mut(plugin_id)
            .and_then(|queue| queue.pop_front())
    }

    fn push_plugin_event(&self, event: PluginRuntimeEvent) {
        if let Ok(mut state) = self.inner.lock() {
            if state.plugin_to_host.len() >= self.outbound_queue_cap {
                state.plugin_to_host.pop_front();
            }
            state.plugin_to_host.push_back(event);
        }
    }

    fn drain_plugin_events(&self, max: usize) -> Vec<PluginRuntimeEvent> {
        if max == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Ok(mut state) = self.inner.lock() {
            for _ in 0..max {
                let Some(item) = state.plugin_to_host.pop_front() else {
                    break;
                };
                out.push(item);
            }
        }
        out
    }
}

fn shared_plugin_event_bus() -> PluginEventBus {
    static SHARED: OnceLock<PluginEventBus> = OnceLock::new();
    SHARED
        .get_or_init(|| PluginEventBus::new(HOST_TO_PLUGIN_QUEUE_CAP, PLUGIN_TO_HOST_QUEUE_CAP))
        .clone()
}

pub fn drain_shared_runtime_events(max: usize) -> Vec<PluginRuntimeEvent> {
    shared_plugin_event_bus().drain_plugin_events(max)
}

pub fn push_shared_host_event_json(plugin_id: &str, event_json: &str) {
    shared_plugin_event_bus().push_host_event(plugin_id, event_json.to_string());
}

pub fn broadcast_shared_host_event_json(event_json: &str) {
    let bus = shared_plugin_event_bus();
    for plugin_id in bus.registered_plugin_ids() {
        bus.push_host_event(&plugin_id, event_json.to_string());
    }
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

extern "C" fn plugin_host_emit_event_json(
    user_data: *mut core::ffi::c_void,
    event_json_utf8: StStr,
) -> StStatus {
    if user_data.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let payload = unsafe { crate::util::ststr_to_string_lossy(event_json_utf8) };
    ctx.event_bus.push_plugin_event(PluginRuntimeEvent {
        plugin_id: ctx.plugin_id.clone(),
        kind: PluginRuntimeKind::Notify,
        payload_json: payload,
    });
    StStatus::ok()
}

extern "C" fn plugin_host_poll_event_json(
    user_data: *mut core::ffi::c_void,
    out_event_json_utf8: *mut StStr,
) -> StStatus {
    if user_data.is_null() || out_event_json_utf8.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let out = match ctx.event_bus.poll_host_event(&ctx.plugin_id) {
        Some(event_json) => alloc_host_owned_ststr(&event_json),
        None => StStr::empty(),
    };
    unsafe {
        *out_event_json_utf8 = out;
    }
    StStatus::ok()
}

extern "C" fn plugin_host_send_control_json(
    user_data: *mut core::ffi::c_void,
    request_json_utf8: StStr,
    out_response_json_utf8: *mut StStr,
) -> StStatus {
    if user_data.is_null() || out_response_json_utf8.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let payload = unsafe { crate::util::ststr_to_string_lossy(request_json_utf8) };
    ctx.event_bus.push_plugin_event(PluginRuntimeEvent {
        plugin_id: ctx.plugin_id.clone(),
        kind: PluginRuntimeKind::Control,
        payload_json: payload,
    });
    let ack_json = r#"{"ok":true}"#;
    unsafe {
        *out_response_json_utf8 = alloc_host_owned_ststr(ack_json);
    }
    StStatus::ok()
}

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
) -> (Box<StHostVTable>, Box<PluginHostCtx>) {
    let bus = shared_plugin_event_bus();
    bus.acquire_plugin(plugin_id);

    let mut host_ctx = Box::new(PluginHostCtx::new(
        plugin_id.to_string(),
        runtime_root
            .to_string_lossy()
            .into_owned()
            .into_bytes()
            .into_boxed_slice(),
        bus,
    ));
    let mut host_vtable = Box::new(*base_host);
    host_vtable.user_data = (&mut *host_ctx) as *mut PluginHostCtx as *mut core::ffi::c_void;
    host_vtable.get_runtime_root_utf8 = Some(plugin_host_runtime_root);
    host_vtable.emit_event_json_utf8 = Some(plugin_host_emit_event_json);
    host_vtable.poll_host_event_json_utf8 = Some(plugin_host_poll_event_json);
    host_vtable.send_control_json_utf8 = Some(plugin_host_send_control_json);
    host_vtable.free_host_str_utf8 = Some(plugin_host_free_str);
    (host_vtable, host_ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_acquire_release_cleans_all_plugin_state() {
        let bus = PluginEventBus::new(16, 16);
        bus.acquire_plugin("dev.test.plugin");
        bus.push_host_event("dev.test.plugin", "{\"t\":1}".to_string());
        assert!(bus.poll_host_event("dev.test.plugin").is_some());

        bus.push_plugin_event(PluginRuntimeEvent {
            plugin_id: "dev.test.plugin".to_string(),
            kind: PluginRuntimeKind::Notify,
            payload_json: "{\"k\":\"v\"}".to_string(),
        });
        bus.push_plugin_event(PluginRuntimeEvent {
            plugin_id: "dev.other.plugin".to_string(),
            kind: PluginRuntimeKind::Notify,
            payload_json: "{\"other\":1}".to_string(),
        });

        bus.release_plugin("dev.test.plugin");
        assert!(bus.registered_plugin_ids().is_empty());
        assert!(bus.poll_host_event("dev.test.plugin").is_none());

        let drained = bus.drain_plugin_events(16);
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].plugin_id, "dev.other.plugin");
    }

    #[test]
    fn plugin_refcount_keeps_queue_until_last_generation_drops() {
        let bus = PluginEventBus::new(16, 16);
        bus.acquire_plugin("dev.test.plugin");
        bus.acquire_plugin("dev.test.plugin");
        assert_eq!(
            bus.registered_plugin_ids(),
            vec!["dev.test.plugin".to_string()]
        );

        bus.release_plugin("dev.test.plugin");
        bus.push_host_event("dev.test.plugin", "{\"t\":2}".to_string());
        assert!(bus.poll_host_event("dev.test.plugin").is_some());

        bus.release_plugin("dev.test.plugin");
        assert!(bus.registered_plugin_ids().is_empty());
        bus.push_host_event("dev.test.plugin", "{\"t\":3}".to_string());
        assert!(bus.poll_host_event("dev.test.plugin").is_none());
    }
}
