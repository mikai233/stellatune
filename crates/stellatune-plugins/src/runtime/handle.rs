use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use arc_swap::ArcSwap;
use crossbeam_channel::Sender;
use stellatune_plugin_api::StHostVTable;
use tracing::{debug, warn};

use crate::default_host_vtable;
use crate::load::{RuntimeLoadReport, RuntimePluginInfo};
use crate::runtime::actor::handlers::acquire_current_module_lease::AcquireCurrentModuleLeaseMessage;
use crate::runtime::actor::handlers::active_plugin_ids::ActivePluginIdsMessage;
use crate::runtime::actor::handlers::cleanup_shadow_copies_now::CleanupShadowCopiesNowMessage;
use crate::runtime::actor::handlers::collect_retired_module_leases_by_refcount::CollectRetiredModuleLeasesByRefcountMessage;
use crate::runtime::actor::handlers::current_module_lease_ref::CurrentModuleLeaseRefMessage;
use crate::runtime::actor::handlers::current_plugin_lease_info::CurrentPluginLeaseInfoMessage;
use crate::runtime::actor::handlers::disabled_plugin_ids::DisabledPluginIdsMessage;
use crate::runtime::actor::handlers::list_active_plugins::ListActivePluginsMessage;
use crate::runtime::actor::handlers::load_dir_additive_filtered::LoadDirAdditiveFilteredMessage;
use crate::runtime::actor::handlers::load_dir_additive_from_state::LoadDirAdditiveFromStateMessage;
use crate::runtime::actor::handlers::plugin_lease_state::PluginLeaseStateMessage;
use crate::runtime::actor::handlers::register_worker_control_sender::RegisterWorkerControlSenderMessage;
use crate::runtime::actor::handlers::reload_dir_detailed_from_state::ReloadDirDetailedFromStateMessage;
use crate::runtime::actor::handlers::reload_dir_filtered::ReloadDirFilteredMessage;
use crate::runtime::actor::handlers::reload_dir_from_state::ReloadDirFromStateMessage;
use crate::runtime::actor::handlers::set_disabled_plugin_ids::SetDisabledPluginIdsMessage;
use crate::runtime::actor::handlers::set_plugin_enabled::SetPluginEnabledMessage;
use crate::runtime::actor::handlers::shutdown_and_cleanup::ShutdownAndCleanupMessage;
use crate::runtime::actor::handlers::unload_plugin::UnloadPluginMessage;
use crate::runtime::actor::{PluginRuntimeActor, spawn_plugin_runtime_actor};
use crate::runtime::introspection::{
    CapabilityDescriptor, CapabilityKind, DecoderCandidate, PluginLeaseInfo, PluginLeaseState,
    RuntimeIntrospectionReadCache,
};
use crate::runtime::messages::WorkerControlMessage;
use crate::runtime::model::{ModuleLease, ModuleLeaseRef, RuntimeSyncReport};

const QUERY_TIMEOUT: Duration = Duration::from_secs(6);
const IO_TIMEOUT: Duration = Duration::from_secs(60);
const SLOW_QUERY_LOG_THRESHOLD: Duration = Duration::from_millis(150);

#[derive(Clone)]
pub struct PluginRuntimeHandle {
    actor_ref: stellatune_runtime::thread_actor::ActorRef<PluginRuntimeActor>,
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
        let introspection_cache = Arc::new(ArcSwap::from_pointee(
            RuntimeIntrospectionReadCache::default(),
        ));
        let actor_introspection_cache = Arc::clone(&introspection_cache);
        let (actor_ref, _actor_join) = spawn_plugin_runtime_actor(host, actor_introspection_cache)
            .expect("failed to spawn plugin runtime actor thread");
        Self {
            actor_ref,
            introspection_cache,
        }
    }

    async fn call<M>(&self, op: &'static str, timeout: Duration, message: M) -> Option<M::Response>
    where
        M: stellatune_runtime::thread_actor::Message,
        PluginRuntimeActor: stellatune_runtime::thread_actor::Handler<M>,
    {
        let started = Instant::now();
        match self.actor_ref.call_async(message, timeout).await {
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
            Err(stellatune_runtime::thread_actor::CallError::MailboxClosed)
            | Err(stellatune_runtime::thread_actor::CallError::ActorStopped) => {
                warn!(op, "plugin runtime actor unavailable");
                None
            }
            Err(stellatune_runtime::thread_actor::CallError::Timeout) => {
                warn!(
                    op,
                    timeout_ms = timeout.as_millis() as u64,
                    "plugin runtime async query timed out"
                );
                None
            }
        }
    }

    async fn call_io<T, M>(&self, op: &'static str, timeout: Duration, message: M) -> Result<T>
    where
        M: stellatune_runtime::thread_actor::Message<Response = anyhow::Result<T>>,
        PluginRuntimeActor: stellatune_runtime::thread_actor::Handler<M>,
    {
        match self.actor_ref.call_async(message, timeout).await {
            Ok(result) => result,
            Err(stellatune_runtime::thread_actor::CallError::MailboxClosed)
            | Err(stellatune_runtime::thread_actor::CallError::ActorStopped) => {
                Err(anyhow!("plugin runtime actor unavailable: {op}"))
            }
            Err(stellatune_runtime::thread_actor::CallError::Timeout) => Err(anyhow!(
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
        self.call(
            "register_worker_control_sender",
            QUERY_TIMEOUT,
            RegisterWorkerControlSenderMessage { plugin_id, sender },
        )
        .await
        .unwrap_or(false)
    }

    pub async fn set_disabled_plugin_ids(&self, disabled_ids: HashSet<String>) {
        let _ = self
            .call(
                "set_disabled_plugin_ids",
                QUERY_TIMEOUT,
                SetDisabledPluginIdsMessage { disabled_ids },
            )
            .await;
    }

    pub async fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return;
        }
        let _ = self
            .call(
                "set_plugin_enabled",
                QUERY_TIMEOUT,
                SetPluginEnabledMessage { plugin_id, enabled },
            )
            .await;
    }

    pub async fn disabled_plugin_ids(&self) -> HashSet<String> {
        self.call(
            "disabled_plugin_ids",
            QUERY_TIMEOUT,
            DisabledPluginIdsMessage,
        )
        .await
        .unwrap_or_default()
    }

    pub async fn list_active_plugins(&self) -> Vec<RuntimePluginInfo> {
        self.call(
            "list_active_plugins",
            QUERY_TIMEOUT,
            ListActivePluginsMessage,
        )
        .await
        .unwrap_or_default()
    }

    pub async fn current_module_lease_ref(&self, plugin_id: &str) -> Option<ModuleLeaseRef> {
        let plugin_id = plugin_id.to_string();
        self.call(
            "current_module_lease_ref",
            QUERY_TIMEOUT,
            CurrentModuleLeaseRefMessage { plugin_id },
        )
        .await
        .flatten()
    }

    pub async fn current_plugin_lease_info(&self, plugin_id: &str) -> Option<PluginLeaseInfo> {
        let plugin_id = plugin_id.to_string();
        self.call(
            "current_plugin_lease_info",
            QUERY_TIMEOUT,
            CurrentPluginLeaseInfoMessage { plugin_id },
        )
        .await
        .flatten()
    }

    pub async fn plugin_lease_state(&self, plugin_id: &str) -> Option<PluginLeaseState> {
        let plugin_id = plugin_id.to_string();
        self.call(
            "plugin_lease_state",
            QUERY_TIMEOUT,
            PluginLeaseStateMessage { plugin_id },
        )
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
        self.call(
            "acquire_current_module_lease",
            QUERY_TIMEOUT,
            AcquireCurrentModuleLeaseMessage { plugin_id },
        )
        .await
        .flatten()
    }

    pub async fn active_plugin_ids(&self) -> Vec<String> {
        self.call("active_plugin_ids", QUERY_TIMEOUT, ActivePluginIdsMessage)
            .await
            .unwrap_or_default()
    }

    pub async fn load_dir_additive_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        self.call_io(
            "load_dir_additive_from_state",
            IO_TIMEOUT,
            LoadDirAdditiveFromStateMessage { dir },
        )
        .await
    }

    pub async fn load_dir_additive_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.call_io(
            "load_dir_additive_filtered",
            IO_TIMEOUT,
            LoadDirAdditiveFilteredMessage { dir, disabled_ids },
        )
        .await
    }

    pub async fn reload_dir_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.call_io(
            "reload_dir_filtered",
            IO_TIMEOUT,
            ReloadDirFilteredMessage { dir, disabled_ids },
        )
        .await
    }

    pub async fn reload_dir_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        self.call_io(
            "reload_dir_from_state",
            IO_TIMEOUT,
            ReloadDirFromStateMessage { dir },
        )
        .await
    }

    pub async fn reload_dir_detailed_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        let dir = dir.as_ref().to_path_buf();
        self.call_io(
            "reload_dir_detailed_from_state",
            IO_TIMEOUT,
            ReloadDirDetailedFromStateMessage { dir },
        )
        .await
    }

    pub async fn unload_plugin(&self, plugin_id: &str) -> RuntimeLoadReport {
        let plugin_id = plugin_id.to_string();
        self.call(
            "unload_plugin",
            IO_TIMEOUT,
            UnloadPluginMessage { plugin_id },
        )
        .await
        .unwrap_or_default()
    }

    pub async fn shutdown_and_cleanup(&self) -> RuntimeLoadReport {
        self.call(
            "shutdown_and_cleanup",
            IO_TIMEOUT,
            ShutdownAndCleanupMessage,
        )
        .await
        .unwrap_or_default()
    }

    pub async fn cleanup_shadow_copies_now(&self) {
        let _ = self
            .call(
                "cleanup_shadow_copies_now",
                IO_TIMEOUT,
                CleanupShadowCopiesNowMessage,
            )
            .await;
    }

    pub async fn collect_retired_module_leases_by_refcount(&self) -> usize {
        self.call(
            "collect_retired_module_leases_by_refcount",
            IO_TIMEOUT,
            CollectRetiredModuleLeasesByRefcountMessage,
        )
        .await
        .unwrap_or(0)
    }
}

pub type SharedPluginRuntimeHandle = PluginRuntimeHandle;

pub fn shared_runtime_service() -> SharedPluginRuntimeHandle {
    static SHARED: OnceLock<SharedPluginRuntimeHandle> = OnceLock::new();
    SHARED
        .get_or_init(|| PluginRuntimeHandle::spawn(default_host_vtable()))
        .clone()
}
