use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use stellatune_plugin_api::StHostVTable;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};

use crate::load::{RuntimeLoadReport, RuntimePluginInfo};
use crate::runtime::actor::run_plugin_runtime_actor;
use crate::runtime::backend_control::BackendControlRequest;
use crate::runtime::introspection::{
    CapabilityDescriptor, CapabilityKind, DecoderCandidate, PluginLeaseInfo, PluginLeaseState,
    RuntimeIntrospectionReadCache,
};
use crate::runtime::messages::{RuntimeActorMessage, WorkerControlMessage};
use crate::runtime::model::{ModuleLease, ModuleLeaseRef, RuntimeSyncReport};
use crate::service::default_host_vtable;

const QUERY_TIMEOUT: Duration = Duration::from_secs(6);
const IO_TIMEOUT: Duration = Duration::from_secs(60);
const SLOW_QUERY_LOG_THRESHOLD: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub struct PluginRuntimeHandle {
    tx: crossbeam_channel::Sender<RuntimeActorMessage>,
    introspection_cache: Arc<ArcSwap<RuntimeIntrospectionReadCache>>,
}

impl PluginRuntimeHandle {
    pub fn new(host: StHostVTable) -> Self {
        Self::spawn(host)
    }

    pub fn new_with_default_host() -> Self {
        Self::spawn(default_host_vtable())
    }

    pub(crate) fn spawn(host: StHostVTable) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<RuntimeActorMessage>();
        let introspection_cache = Arc::new(ArcSwap::from_pointee(
            RuntimeIntrospectionReadCache::default(),
        ));
        let actor_introspection_cache = Arc::clone(&introspection_cache);
        let _actor_join = thread::Builder::new()
            .name("stellatune-plugin-runtime-actor".to_string())
            .spawn(move || {
                run_plugin_runtime_actor(rx, host, actor_introspection_cache);
            })
            .expect("failed to spawn plugin runtime actor thread");
        Self {
            tx,
            introspection_cache,
        }
    }

    async fn send<T: Send + 'static>(
        &self,
        op: &'static str,
        timeout: Duration,
        build: impl FnOnce(oneshot::Sender<T>) -> RuntimeActorMessage,
    ) -> Option<T> {
        let started = Instant::now();
        let (resp_tx, resp_rx) = oneshot::channel();
        if self.tx.send(build(resp_tx)).is_err() {
            warn!(op, "plugin runtime actor request channel disconnected");
            return None;
        }
        match tokio::time::timeout(timeout, resp_rx).await {
            Ok(Ok(value)) => {
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
                    timeout_ms = timeout.as_millis() as u64,
                    "plugin runtime async query timed out"
                );
                None
            }
        }
    }

    async fn recv_result<T>(
        rx: oneshot::Receiver<anyhow::Result<T>>,
        op: &'static str,
        timeout: Duration,
    ) -> Result<T> {
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(anyhow!("plugin runtime actor dropped response: {op}")),
            Err(_) => Err(anyhow!(
                "plugin runtime actor response timed out: {op} ({timeout_ms}ms)",
                timeout_ms = timeout.as_millis() as u64
            )),
        }
    }

    pub(crate) async fn register_worker_control_sender(
        &self,
        plugin_id: &str,
        sender: Sender<WorkerControlMessage>,
    ) -> bool {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return false;
        }
        self.send(
            "register_worker_control_sender",
            QUERY_TIMEOUT,
            move |resp_tx| RuntimeActorMessage::RegisterWorkerControlSender {
                plugin_id,
                sender,
                resp_tx,
            },
        )
        .await
        .unwrap_or(false)
    }

    pub async fn subscribe_backend_control_requests(
        &self,
    ) -> mpsc::UnboundedReceiver<BackendControlRequest> {
        self.send(
            "subscribe_backend_control_requests",
            QUERY_TIMEOUT,
            |resp_tx| RuntimeActorMessage::SubscribeBackendControlRequests { resp_tx },
        )
        .await
        .unwrap_or_else(|| {
            let (_tx, rx) = mpsc::unbounded_channel();
            rx
        })
    }

    pub async fn set_disabled_plugin_ids(&self, disabled_ids: HashSet<String>) {
        let _ = self
            .send("set_disabled_plugin_ids", QUERY_TIMEOUT, move |resp_tx| {
                RuntimeActorMessage::SetDisabledPluginIds {
                    disabled_ids,
                    resp_tx,
                }
            })
            .await;
    }

    pub async fn push_host_event_json(&self, plugin_id: &str, event_json: &str) {
        let plugin_id = plugin_id.to_string();
        let event_json = event_json.to_string();
        let _ = self
            .send("push_host_event_json", QUERY_TIMEOUT, move |resp_tx| {
                RuntimeActorMessage::PushHostEventJson {
                    plugin_id,
                    event_json,
                    resp_tx,
                }
            })
            .await;
    }

    pub async fn broadcast_host_event_json(&self, event_json: &str) {
        let event_json = event_json.to_string();
        let _ = self
            .send("broadcast_host_event_json", QUERY_TIMEOUT, move |resp_tx| {
                RuntimeActorMessage::BroadcastHostEventJson {
                    event_json,
                    resp_tx,
                }
            })
            .await;
    }

    pub async fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return;
        }
        let _ = self
            .send("set_plugin_enabled", QUERY_TIMEOUT, move |resp_tx| {
                RuntimeActorMessage::SetPluginEnabled {
                    plugin_id,
                    enabled,
                    resp_tx,
                }
            })
            .await;
    }

    pub async fn disabled_plugin_ids(&self) -> HashSet<String> {
        self.send("disabled_plugin_ids", QUERY_TIMEOUT, |resp_tx| {
            RuntimeActorMessage::DisabledPluginIds { resp_tx }
        })
        .await
        .unwrap_or_default()
    }

    pub async fn list_active_plugins(&self) -> Vec<RuntimePluginInfo> {
        self.send("list_active_plugins", QUERY_TIMEOUT, |resp_tx| {
            RuntimeActorMessage::ListActivePlugins { resp_tx }
        })
        .await
        .unwrap_or_default()
    }

    pub async fn current_module_lease_ref(&self, plugin_id: &str) -> Option<ModuleLeaseRef> {
        let plugin_id = plugin_id.to_string();
        self.send("current_module_lease_ref", QUERY_TIMEOUT, move |resp_tx| {
            RuntimeActorMessage::CurrentModuleLeaseRef { plugin_id, resp_tx }
        })
        .await
        .flatten()
    }

    pub async fn current_plugin_lease_info(&self, plugin_id: &str) -> Option<PluginLeaseInfo> {
        let plugin_id = plugin_id.to_string();
        self.send("current_plugin_lease_info", QUERY_TIMEOUT, move |resp_tx| {
            RuntimeActorMessage::CurrentPluginLeaseInfo { plugin_id, resp_tx }
        })
        .await
        .flatten()
    }

    pub async fn plugin_lease_state(&self, plugin_id: &str) -> Option<PluginLeaseState> {
        let plugin_id = plugin_id.to_string();
        self.send("plugin_lease_state", QUERY_TIMEOUT, move |resp_tx| {
            RuntimeActorMessage::PluginLeaseState { plugin_id, resp_tx }
        })
        .await
        .flatten()
    }

    pub async fn list_capabilities(&self, plugin_id: &str) -> Vec<CapabilityDescriptor> {
        self.introspection_cache.load().list_capabilities(plugin_id)
    }

    pub async fn find_capability(
        &self,
        plugin_id: &str,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Option<CapabilityDescriptor> {
        self.introspection_cache
            .load()
            .find_capability(plugin_id, kind, type_id)
    }

    pub async fn list_decoder_candidates_for_ext(&self, ext: &str) -> Vec<DecoderCandidate> {
        self.introspection_cache
            .load()
            .list_decoder_candidates_for_ext(ext)
    }

    pub(crate) async fn acquire_current_module_lease(
        &self,
        plugin_id: &str,
    ) -> Option<Arc<ModuleLease>> {
        let plugin_id = plugin_id.to_string();
        self.send(
            "acquire_current_module_lease",
            QUERY_TIMEOUT,
            move |resp_tx| RuntimeActorMessage::AcquireCurrentModuleLease { plugin_id, resp_tx },
        )
        .await
        .flatten()
    }

    pub async fn active_plugin_ids(&self) -> Vec<String> {
        self.send("active_plugin_ids", QUERY_TIMEOUT, |resp_tx| {
            RuntimeActorMessage::ActivePluginIds { resp_tx }
        })
        .await
        .unwrap_or_default()
    }

    pub async fn load_dir_additive_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(RuntimeActorMessage::LoadDirAdditiveFromState { dir, resp_tx })
            .map_err(|_| anyhow!("plugin runtime actor unavailable"))?;
        Self::recv_result(resp_rx, "load_dir_additive_from_state", IO_TIMEOUT).await
    }

    pub async fn load_dir_additive_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(RuntimeActorMessage::LoadDirAdditiveFiltered {
                dir,
                disabled_ids,
                resp_tx,
            })
            .map_err(|_| anyhow!("plugin runtime actor unavailable"))?;
        Self::recv_result(resp_rx, "load_dir_additive_filtered", IO_TIMEOUT).await
    }

    pub async fn reload_dir_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(RuntimeActorMessage::ReloadDirFiltered {
                dir: dir_path,
                disabled_ids,
                resp_tx,
            })
            .map_err(|_| anyhow!("plugin runtime actor unavailable"))?;
        Self::recv_result(resp_rx, "reload_dir_filtered", IO_TIMEOUT).await
    }

    pub async fn reload_dir_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(RuntimeActorMessage::ReloadDirFromState {
                dir: dir_path,
                resp_tx,
            })
            .map_err(|_| anyhow!("plugin runtime actor unavailable"))?;
        Self::recv_result(resp_rx, "reload_dir_from_state", IO_TIMEOUT).await
    }

    pub async fn reload_dir_detailed_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let (resp_tx, resp_rx) = oneshot::channel();
        self.tx
            .send(RuntimeActorMessage::ReloadDirDetailedFromState {
                dir: dir_path,
                resp_tx,
            })
            .map_err(|_| anyhow!("plugin runtime actor unavailable"))?;
        Self::recv_result(resp_rx, "reload_dir_detailed_from_state", IO_TIMEOUT).await
    }

    pub async fn unload_plugin(&self, plugin_id: &str) -> RuntimeLoadReport {
        let plugin_id = plugin_id.to_string();
        self.send("unload_plugin", IO_TIMEOUT, move |resp_tx| {
            RuntimeActorMessage::UnloadPlugin { plugin_id, resp_tx }
        })
        .await
        .unwrap_or_default()
    }

    pub async fn shutdown_and_cleanup(&self) -> RuntimeLoadReport {
        self.send("shutdown_and_cleanup", IO_TIMEOUT, |resp_tx| {
            RuntimeActorMessage::ShutdownAndCleanup { resp_tx }
        })
        .await
        .unwrap_or_default()
    }

    pub async fn cleanup_shadow_copies_now(&self) {
        let _ = self
            .send("cleanup_shadow_copies_now", IO_TIMEOUT, |resp_tx| {
                RuntimeActorMessage::CleanupShadowCopiesNow { resp_tx }
            })
            .await;
    }

    pub async fn collect_retired_module_leases_by_refcount(&self) -> usize {
        self.send(
            "collect_retired_module_leases_by_refcount",
            IO_TIMEOUT,
            |resp_tx| RuntimeActorMessage::CollectRetiredModuleLeasesByRefcount { resp_tx },
        )
        .await
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
