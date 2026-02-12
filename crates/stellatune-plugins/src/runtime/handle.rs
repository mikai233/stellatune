use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::thread;

use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender};
use stellatune_plugin_api::StHostVTable;

use crate::load::{RuntimeLoadReport, RuntimePluginInfo};
use crate::runtime::actor::{
    RuntimeActorState, RuntimeActorTask, WorkerControlMessage, run_plugin_runtime_actor,
};
use crate::runtime::backend_control::BackendControlRequest;
use crate::runtime::model::{ModuleLease, ModuleLeaseRef, RuntimeSyncReport};
use crate::service::default_host_vtable;

#[derive(Clone)]
pub struct PluginRuntimeHandle {
    tx: Sender<RuntimeActorTask>,
}

impl PluginRuntimeHandle {
    pub(crate) fn spawn(host: StHostVTable) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<RuntimeActorTask>();
        thread::Builder::new()
            .name("stellatune-plugin-runtime".to_string())
            .spawn(move || run_plugin_runtime_actor(rx, host))
            .expect("failed to spawn plugin runtime actor");
        Self { tx }
    }

    fn exec_value<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Option<R> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        let task: RuntimeActorTask = Box::new(move |state| {
            let out = f(state);
            let _ = resp_tx.send(out);
        });
        self.tx.send(task).ok()?;
        resp_rx.recv().ok()
    }

    fn exec_result<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Result<R> {
        self.exec_value(f)
            .ok_or_else(|| anyhow!("plugin runtime actor unavailable"))
    }

    pub fn register_worker_control_sender(
        &self,
        plugin_id: &str,
        sender: Sender<WorkerControlMessage>,
    ) -> bool {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return false;
        }
        self.exec_value(move |state| {
            state.register_worker_control_sender(plugin_id, sender);
        })
        .is_some()
    }

    pub fn subscribe_worker_control(&self, plugin_id: &str) -> Receiver<WorkerControlMessage> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let _ = self.register_worker_control_sender(plugin_id, tx);
        rx
    }

    pub fn subscribe_backend_control_requests(&self) -> Receiver<BackendControlRequest> {
        self.exec_value(|state| state.service.subscribe_backend_control_requests())
            .unwrap_or_else(|| {
                let (_tx, rx) = crossbeam_channel::unbounded();
                rx
            })
    }

    pub fn set_disabled_plugin_ids(&self, disabled_ids: HashSet<String>) {
        let _ = self.exec_value(move |state| {
            state.service.set_disabled_plugin_ids(disabled_ids);
        });
    }

    pub fn push_host_event_json(&self, plugin_id: &str, event_json: &str) {
        let plugin_id = plugin_id.to_string();
        let event_json = event_json.to_string();
        let _ = self.exec_value(move |state| {
            state.service.push_host_event_json(&plugin_id, &event_json);
        });
    }

    pub fn broadcast_host_event_json(&self, event_json: &str) {
        let event_json = event_json.to_string();
        let _ = self.exec_value(move |state| {
            state.service.broadcast_host_event_json(&event_json);
        });
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return;
        }
        let _ = self.exec_value(move |state| {
            state.service.set_plugin_enabled(&plugin_id, enabled);
            if !enabled {
                state.emit_worker_destroy(&plugin_id, "plugin disabled");
            }
        });
    }

    pub fn disabled_plugin_ids(&self) -> HashSet<String> {
        self.exec_value(|state| state.service.disabled_plugin_ids())
            .unwrap_or_default()
    }

    pub fn list_active_plugins(&self) -> Vec<RuntimePluginInfo> {
        self.exec_value(|state| state.service.list_active_plugins())
            .unwrap_or_default()
    }

    pub fn current_module_lease_ref(&self, plugin_id: &str) -> Option<ModuleLeaseRef> {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| state.service.current_module_lease_ref(&plugin_id))
            .flatten()
    }

    pub(crate) fn acquire_current_module_lease(&self, plugin_id: &str) -> Option<Arc<ModuleLease>> {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| state.service.acquire_current_module_lease(&plugin_id))
            .flatten()
    }

    pub fn active_plugin_ids(&self) -> Vec<String> {
        self.exec_value(|state| state.service.active_plugin_ids())
            .unwrap_or_default()
    }

    pub fn load_dir_additive_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        self.exec_result(move |state| state.service.load_dir_additive_from_state(&dir))?
    }

    pub fn load_dir_additive_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.exec_result(move |state| {
            state
                .service
                .load_dir_additive_filtered(&dir, &disabled_ids)
        })?
    }

    pub fn reload_dir_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.exec_result(move |state| {
            let report = state.service.reload_dir_filtered(&dir_path, &disabled_ids);
            if let Ok(success) = &report {
                for plugin in &success.loaded {
                    state.emit_worker_recreate(&plugin.id, "plugin lease swapped");
                }
                for plugin_id in &success.deactivated {
                    state.emit_worker_destroy(plugin_id, "plugin deactivated");
                }
            }
            report
        })?
    }

    pub fn reload_dir_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        self.exec_result(move |state| {
            let report = state.service.reload_dir_from_state(&dir_path);
            if let Ok(success) = &report {
                for plugin in &success.loaded {
                    state.emit_worker_recreate(&plugin.id, "plugin lease swapped");
                }
                for plugin_id in &success.deactivated {
                    state.emit_worker_destroy(plugin_id, "plugin deactivated");
                }
            }
            report
        })?
    }

    pub fn reload_dir_detailed_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        let dir_path = dir.as_ref().to_path_buf();
        self.exec_result(move |state| {
            let report = state.service.reload_dir_detailed_from_state(&dir_path);
            if let Ok(success) = &report {
                for plugin in &success.load_report.loaded {
                    state.emit_worker_recreate(&plugin.id, "plugin lease swapped");
                }
                for plugin_id in &success.load_report.deactivated {
                    state.emit_worker_destroy(plugin_id, "plugin deactivated");
                }
            }
            report
        })?
    }

    pub fn unload_plugin(&self, plugin_id: &str) -> RuntimeLoadReport {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| {
            let report = state.service.unload_plugin(&plugin_id);
            state.emit_worker_destroy(&plugin_id, "plugin unloaded");
            report
        })
        .unwrap_or_default()
    }

    pub fn shutdown_and_cleanup(&self) -> RuntimeLoadReport {
        self.exec_value(|state| {
            let report = state.service.shutdown_and_cleanup();
            state.emit_worker_destroy_all("plugin runtime shutdown");
            report
        })
        .unwrap_or_default()
    }

    pub fn cleanup_shadow_copies_now(&self) {
        let _ = self.exec_value(|state| {
            state.service.cleanup_shadow_copies_now();
        });
    }

    pub fn collect_retired_module_leases_by_refcount(&self) -> usize {
        self.exec_value(|state| state.service.collect_retired_module_leases_by_refcount())
            .unwrap_or(0)
    }
}

pub type SharedPluginRuntimeService = PluginRuntimeHandle;

pub fn shared_runtime_service() -> SharedPluginRuntimeService {
    static SHARED: OnceLock<SharedPluginRuntimeService> = OnceLock::new();
    SHARED
        .get_or_init(|| PluginRuntimeHandle::spawn(default_host_vtable()))
        .clone()
}
