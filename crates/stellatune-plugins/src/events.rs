use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use stellatune_plugin_api::StHostVTable;

#[derive(Debug, Default)]
struct PluginEventBusState {
    plugin_ref_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct PluginEventBus {
    inner: Arc<Mutex<PluginEventBusState>>,
}

impl PluginEventBus {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PluginEventBusState::default())),
        }
    }

    pub(crate) fn acquire_plugin(&self, plugin_id: &str) {
        if let Ok(mut state) = self.inner.lock() {
            let count = state
                .plugin_ref_counts
                .entry(plugin_id.to_string())
                .or_insert(0);
            *count = count.saturating_add(1);
        }
    }

    pub(crate) fn release_plugin(&self, plugin_id: &str) {
        if let Ok(mut state) = self.inner.lock() {
            let should_drop = match state.plugin_ref_counts.get_mut(plugin_id) {
                Some(count) => {
                    *count = count.saturating_sub(1);
                    *count == 0
                },
                None => false,
            };
            if should_drop {
                state.plugin_ref_counts.remove(plugin_id);
            }
        }
    }
}

pub(crate) fn new_runtime_event_bus() -> PluginEventBus {
    PluginEventBus::new()
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

extern "C" fn plugin_host_runtime_root(
    user_data: *mut core::ffi::c_void,
) -> stellatune_plugin_api::StStr {
    if user_data.is_null() {
        return stellatune_plugin_api::StStr::empty();
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    if ctx.runtime_root_utf8.is_empty() {
        return stellatune_plugin_api::StStr::empty();
    }
    stellatune_plugin_api::StStr {
        ptr: ctx.runtime_root_utf8.as_ptr(),
        len: ctx.runtime_root_utf8.len(),
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
    host_vtable.begin_poll_host_event_json_utf8 = None;
    host_vtable.begin_send_control_json_utf8 = None;
    host_vtable.free_host_str_utf8 = None;
    (host_vtable, host_ctx)
}
