use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender};
use stellatune_plugin_api::StHostVTable;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};

use crate::load::{RuntimeLoadReport, RuntimePluginInfo};
use crate::runtime::actor::{
    RuntimeActorState, RuntimeActorTask, WorkerControlMessage, run_plugin_runtime_actor,
};
use crate::runtime::backend_control::BackendControlRequest;
use crate::runtime::introspection::{
    CapabilityDescriptor, CapabilityKind, DecoderCandidate, PluginLeaseInfo, PluginLeaseState,
};
use crate::runtime::model::{ModuleLease, ModuleLeaseRef, RuntimeSyncReport};
use crate::service::default_host_vtable;
use stellatune_runtime as global_runtime;

const QUERY_TIMEOUT: Duration = Duration::from_secs(6);
const SLOW_QUERY_LOG_THRESHOLD: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub struct PluginRuntimeHandle {
    tx: mpsc::UnboundedSender<RuntimeActorTask>,
}

fn load_dir_additive_from_state_blocking(
    state: &mut RuntimeActorState,
    dir: &Path,
) -> Result<RuntimeLoadReport> {
    tokio::task::block_in_place(|| state.service.load_dir_additive_from_state(dir))
}

fn load_dir_additive_filtered_blocking(
    state: &mut RuntimeActorState,
    dir: &Path,
    disabled_ids: &HashSet<String>,
) -> Result<RuntimeLoadReport> {
    tokio::task::block_in_place(|| state.service.load_dir_additive_filtered(dir, disabled_ids))
}

fn reload_dir_filtered_blocking(
    state: &mut RuntimeActorState,
    dir: &Path,
    disabled_ids: &HashSet<String>,
) -> Result<RuntimeLoadReport> {
    let report =
        tokio::task::block_in_place(|| state.service.reload_dir_filtered(dir, disabled_ids));
    if let Ok(success) = &report {
        for plugin in &success.loaded {
            state.emit_worker_recreate(&plugin.id, "plugin lease swapped");
        }
        for plugin_id in &success.deactivated {
            state.emit_worker_destroy(plugin_id, "plugin deactivated");
        }
    }
    report
}

fn reload_dir_from_state_blocking(
    state: &mut RuntimeActorState,
    dir: &Path,
) -> Result<RuntimeLoadReport> {
    let report = tokio::task::block_in_place(|| state.service.reload_dir_from_state(dir));
    if let Ok(success) = &report {
        for plugin in &success.loaded {
            state.emit_worker_recreate(&plugin.id, "plugin lease swapped");
        }
        for plugin_id in &success.deactivated {
            state.emit_worker_destroy(plugin_id, "plugin deactivated");
        }
    }
    report
}

fn reload_dir_detailed_from_state_blocking(
    state: &mut RuntimeActorState,
    dir: &Path,
) -> Result<RuntimeSyncReport> {
    let report = tokio::task::block_in_place(|| state.service.reload_dir_detailed_from_state(dir));
    if let Ok(success) = &report {
        for plugin in &success.load_report.loaded {
            state.emit_worker_recreate(&plugin.id, "plugin lease swapped");
        }
        for plugin_id in &success.load_report.deactivated {
            state.emit_worker_destroy(plugin_id, "plugin deactivated");
        }
    }
    report
}

impl PluginRuntimeHandle {
    pub fn new(host: StHostVTable) -> Self {
        Self::spawn(host)
    }

    pub fn new_with_default_host() -> Self {
        Self::spawn(default_host_vtable())
    }

    pub(crate) fn spawn(host: StHostVTable) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<RuntimeActorTask>();
        global_runtime::spawn(async move {
            run_plugin_runtime_actor(rx, host).await;
        });
        Self { tx }
    }

    fn exec_value<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Option<R> {
        let (resp_tx, resp_rx) = std::sync::mpsc::sync_channel(1);
        let task: RuntimeActorTask = Box::new(move |state| {
            let out = f(state);
            let _ = resp_tx.send(out);
        });
        self.tx.send(task).ok()?;
        resp_rx.recv().ok()
    }

    fn exec_value_quick<R: Send + 'static>(
        &self,
        op: &'static str,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Option<R> {
        let started = Instant::now();
        let (resp_tx, resp_rx) = std::sync::mpsc::sync_channel(1);
        let task: RuntimeActorTask = Box::new(move |state| {
            let out = f(state);
            let _ = resp_tx.send(out);
        });
        self.tx.send(task).ok()?;
        match resp_rx.recv_timeout(QUERY_TIMEOUT) {
            Ok(value) => {
                let elapsed = started.elapsed();
                if elapsed >= SLOW_QUERY_LOG_THRESHOLD {
                    debug!(
                        op,
                        elapsed_ms = elapsed.as_millis() as u64,
                        "plugin runtime query completed slowly"
                    );
                }
                Some(value)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                warn!(
                    op,
                    timeout_ms = QUERY_TIMEOUT.as_millis() as u64,
                    "plugin runtime query timed out"
                );
                None
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                warn!(op, "plugin runtime query response channel disconnected");
                None
            }
        }
    }

    async fn exec_value_quick_async<R: Send + 'static>(
        &self,
        op: &'static str,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Option<R> {
        let started = Instant::now();
        let (resp_tx, resp_rx) = oneshot::channel();
        let task: RuntimeActorTask = Box::new(move |state| {
            let out = f(state);
            let _ = resp_tx.send(out);
        });
        self.tx.send(task).ok()?;
        match tokio::time::timeout(QUERY_TIMEOUT, resp_rx).await {
            Ok(Ok(value)) => {
                let elapsed = started.elapsed();
                if elapsed >= SLOW_QUERY_LOG_THRESHOLD {
                    debug!(
                        op,
                        elapsed_ms = elapsed.as_millis() as u64,
                        "plugin runtime async query completed slowly"
                    );
                }
                Some(value)
            }
            Ok(Err(_)) => {
                warn!(
                    op,
                    "plugin runtime async query response channel disconnected"
                );
                None
            }
            Err(_) => {
                warn!(
                    op,
                    timeout_ms = QUERY_TIMEOUT.as_millis() as u64,
                    "plugin runtime async query timed out"
                );
                None
            }
        }
    }

    async fn exec_value_async<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Option<R> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let task: RuntimeActorTask = Box::new(move |state| {
            let out = f(state);
            let _ = resp_tx.send(out);
        });
        self.tx.send(task).ok()?;
        resp_rx.await.ok()
    }

    fn exec_result<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Result<R> {
        self.exec_value(f)
            .ok_or_else(|| anyhow!("plugin runtime actor unavailable"))
    }

    async fn exec_result_async<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Result<R> {
        self.exec_value_async(f)
            .await
            .ok_or_else(|| anyhow!("plugin runtime actor unavailable"))
    }

    pub(crate) fn register_worker_control_sender(
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

    pub async fn set_disabled_plugin_ids_async(&self, disabled_ids: HashSet<String>) {
        let _ = self
            .exec_value_async(move |state| {
                state.service.set_disabled_plugin_ids(disabled_ids);
            })
            .await;
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
        self.exec_value_quick("disabled_plugin_ids", |state| {
            state.service.disabled_plugin_ids()
        })
        .unwrap_or_default()
    }

    pub fn list_active_plugins(&self) -> Vec<RuntimePluginInfo> {
        self.exec_value_quick("list_active_plugins", |state| {
            state.service.list_active_plugins()
        })
        .unwrap_or_default()
    }

    pub fn current_module_lease_ref(&self, plugin_id: &str) -> Option<ModuleLeaseRef> {
        let plugin_id = plugin_id.to_string();
        self.exec_value_quick("current_module_lease_ref", move |state| {
            state.service.current_module_lease_ref(&plugin_id)
        })
        .flatten()
    }

    pub fn current_plugin_lease_info(&self, plugin_id: &str) -> Option<PluginLeaseInfo> {
        let plugin_id = plugin_id.to_string();
        self.exec_value_quick("current_plugin_lease_info", move |state| {
            state.service.current_plugin_lease_info(&plugin_id)
        })
        .flatten()
    }

    pub async fn current_plugin_lease_info_async(
        &self,
        plugin_id: &str,
    ) -> Option<PluginLeaseInfo> {
        let plugin_id = plugin_id.to_string();
        self.exec_value_quick_async("current_plugin_lease_info_async", move |state| {
            state.service.current_plugin_lease_info(&plugin_id)
        })
        .await
        .flatten()
    }

    pub fn plugin_lease_state(&self, plugin_id: &str) -> Option<PluginLeaseState> {
        let plugin_id = plugin_id.to_string();
        self.exec_value_quick("plugin_lease_state", move |state| {
            state.service.plugin_lease_state(&plugin_id)
        })
        .flatten()
    }

    pub fn list_capabilities(&self, plugin_id: &str) -> Vec<CapabilityDescriptor> {
        let plugin_id = plugin_id.to_string();
        self.exec_value_quick("list_capabilities", move |state| {
            state.service.list_capabilities(&plugin_id)
        })
        .unwrap_or_default()
    }

    pub async fn list_capabilities_async(&self, plugin_id: &str) -> Vec<CapabilityDescriptor> {
        let plugin_id = plugin_id.to_string();
        self.exec_value_quick_async("list_capabilities_async", move |state| {
            state.service.list_capabilities(&plugin_id)
        })
        .await
        .unwrap_or_default()
    }

    pub fn find_capability(
        &self,
        plugin_id: &str,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Option<CapabilityDescriptor> {
        let plugin_id = plugin_id.to_string();
        let type_id = type_id.to_string();
        self.exec_value_quick("find_capability", move |state| {
            state.service.find_capability(&plugin_id, kind, &type_id)
        })
        .flatten()
    }

    pub fn list_decoder_candidates_for_ext(&self, ext: &str) -> Vec<DecoderCandidate> {
        let ext = ext.to_string();
        self.exec_value_quick("list_decoder_candidates_for_ext", move |state| {
            state.service.list_decoder_candidates_for_ext(&ext)
        })
        .unwrap_or_default()
    }

    pub(crate) fn acquire_current_module_lease(&self, plugin_id: &str) -> Option<Arc<ModuleLease>> {
        let plugin_id = plugin_id.to_string();
        self.exec_value_quick("acquire_current_module_lease", move |state| {
            state.service.acquire_current_module_lease(&plugin_id)
        })
        .flatten()
    }

    pub fn active_plugin_ids(&self) -> Vec<String> {
        self.exec_value_quick("active_plugin_ids", |state| {
            state.service.active_plugin_ids()
        })
        .unwrap_or_default()
    }

    pub async fn active_plugin_ids_async(&self) -> Vec<String> {
        self.exec_value_quick_async("active_plugin_ids_async", |state| {
            state.service.active_plugin_ids()
        })
        .await
        .unwrap_or_default()
    }

    pub fn load_dir_additive_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        self.exec_result(move |state| load_dir_additive_from_state_blocking(state, &dir))?
    }

    pub async fn load_dir_additive_from_state_async(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        self.exec_result_async(move |state| load_dir_additive_from_state_blocking(state, &dir))
            .await?
    }

    pub fn load_dir_additive_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.exec_result(move |state| {
            load_dir_additive_filtered_blocking(state, &dir, &disabled_ids)
        })?
    }

    pub async fn load_dir_additive_filtered_async(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.exec_result_async(move |state| {
            load_dir_additive_filtered_blocking(state, &dir, &disabled_ids)
        })
        .await?
    }

    pub fn reload_dir_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.exec_result(move |state| {
            reload_dir_filtered_blocking(state, &dir_path, &disabled_ids)
        })?
    }

    pub async fn reload_dir_filtered_async(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.exec_result_async(move |state| {
            reload_dir_filtered_blocking(state, &dir_path, &disabled_ids)
        })
        .await?
    }

    pub fn reload_dir_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        self.exec_result(move |state| reload_dir_from_state_blocking(state, &dir_path))?
    }

    pub async fn reload_dir_from_state_async(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        self.exec_result_async(move |state| reload_dir_from_state_blocking(state, &dir_path))
            .await?
    }

    pub fn reload_dir_detailed_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        let dir_path = dir.as_ref().to_path_buf();
        self.exec_result(move |state| reload_dir_detailed_from_state_blocking(state, &dir_path))?
    }

    pub async fn reload_dir_detailed_from_state_async(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        let dir_path = dir.as_ref().to_path_buf();
        self.exec_result_async(move |state| {
            reload_dir_detailed_from_state_blocking(state, &dir_path)
        })
        .await?
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
