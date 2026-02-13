use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use arc_swap::ArcSwap;
use crossbeam_channel::{Receiver, Sender};
use stellatune_plugin_api::StHostVTable;
use tracing::debug;

use crate::load::RuntimeLoadReport;
use crate::runtime::introspection::RuntimeIntrospectionReadCache;
use crate::runtime::messages::{RuntimeActorMessage, WorkerControlMessage};
use crate::service::PluginRuntimeService;

pub(crate) struct RuntimeActorState {
    pub(crate) service: PluginRuntimeService,
    pub(crate) worker_control_subscribers: HashMap<String, Vec<Sender<WorkerControlMessage>>>,
    worker_control_seq: HashMap<String, u64>,
    introspection_cache: Arc<ArcSwap<RuntimeIntrospectionReadCache>>,
}

impl RuntimeActorState {
    pub(crate) fn new(
        host: StHostVTable,
        introspection_cache: Arc<ArcSwap<RuntimeIntrospectionReadCache>>,
    ) -> Self {
        let state = Self {
            service: PluginRuntimeService::new(host),
            worker_control_subscribers: HashMap::new(),
            worker_control_seq: HashMap::new(),
            introspection_cache,
        };
        state.refresh_introspection_cache_snapshot();
        state
    }

    pub(crate) fn register_worker_control_sender(
        &mut self,
        plugin_id: String,
        sender: Sender<WorkerControlMessage>,
    ) {
        self.worker_control_subscribers
            .entry(plugin_id)
            .or_default()
            .push(sender);
    }

    fn next_worker_control_seq(&mut self, plugin_id: &str) -> u64 {
        let seq = self
            .worker_control_seq
            .entry(plugin_id.to_string())
            .or_insert(0);
        *seq = seq.saturating_add(1);
        *seq
    }

    fn emit_worker_control(&mut self, plugin_id: &str, message: WorkerControlMessage) {
        let mut should_remove = false;
        if let Some(subscribers) = self.worker_control_subscribers.get_mut(plugin_id) {
            subscribers.retain(|tx| tx.send(message.clone()).is_ok());
            should_remove = subscribers.is_empty();
        }
        if should_remove {
            self.worker_control_subscribers.remove(plugin_id);
        }
    }

    pub(crate) fn emit_worker_recreate(&mut self, plugin_id: &str, reason: &str) {
        let seq = self.next_worker_control_seq(plugin_id);
        self.emit_worker_control(
            plugin_id,
            WorkerControlMessage::Recreate {
                reason: reason.to_string(),
                seq,
            },
        );
    }

    pub(crate) fn emit_worker_destroy(&mut self, plugin_id: &str, reason: &str) {
        let seq = self.next_worker_control_seq(plugin_id);
        self.emit_worker_control(
            plugin_id,
            WorkerControlMessage::Destroy {
                reason: reason.to_string(),
                seq,
            },
        );
    }

    pub(crate) fn emit_worker_destroy_all(&mut self, reason: &str) {
        let plugin_ids = self
            .worker_control_subscribers
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for plugin_id in plugin_ids {
            self.emit_worker_destroy(&plugin_id, reason);
        }
    }

    fn emit_reload_notifications(&mut self, report: &RuntimeLoadReport) {
        for plugin in &report.loaded {
            self.emit_worker_recreate(&plugin.id, "plugin lease swapped");
        }
        for plugin_id in &report.deactivated {
            self.emit_worker_destroy(plugin_id, "plugin deactivated");
        }
    }

    fn refresh_introspection_cache_snapshot(&self) {
        self.introspection_cache
            .store(self.service.introspection_cache_snapshot());
    }

    fn handle_message(&mut self, message: RuntimeActorMessage) {
        let msg = message_name(&message);
        let started = Instant::now();
        debug!(message = msg, "plugin runtime actor handling message");
        let mut should_refresh_introspection_cache = false;
        match message {
            RuntimeActorMessage::RegisterWorkerControlSender {
                plugin_id,
                sender,
                resp_tx,
            } => {
                self.register_worker_control_sender(plugin_id, sender);
                let _ = resp_tx.send(true);
            }
            RuntimeActorMessage::SubscribeBackendControlRequests { resp_tx } => {
                let _ = resp_tx.send(self.service.subscribe_backend_control_requests());
            }
            RuntimeActorMessage::SetDisabledPluginIds {
                disabled_ids,
                resp_tx,
            } => {
                self.service.set_disabled_plugin_ids(disabled_ids);
                let _ = resp_tx.send(());
            }
            RuntimeActorMessage::PushHostEventJson {
                plugin_id,
                event_json,
                resp_tx,
            } => {
                self.service.push_host_event_json(&plugin_id, &event_json);
                let _ = resp_tx.send(());
            }
            RuntimeActorMessage::BroadcastHostEventJson {
                event_json,
                resp_tx,
            } => {
                self.service.broadcast_host_event_json(&event_json);
                let _ = resp_tx.send(());
            }
            RuntimeActorMessage::SetPluginEnabled {
                plugin_id,
                enabled,
                resp_tx,
            } => {
                self.service.set_plugin_enabled(&plugin_id, enabled);
                let _ = resp_tx.send(());
            }
            RuntimeActorMessage::DisabledPluginIds { resp_tx } => {
                let _ = resp_tx.send(self.service.disabled_plugin_ids());
            }
            RuntimeActorMessage::ListActivePlugins { resp_tx } => {
                let _ = resp_tx.send(self.service.list_active_plugins());
            }
            RuntimeActorMessage::CurrentModuleLeaseRef { plugin_id, resp_tx } => {
                let _ = resp_tx.send(self.service.current_module_lease_ref(&plugin_id));
            }
            RuntimeActorMessage::CurrentPluginLeaseInfo { plugin_id, resp_tx } => {
                let _ = resp_tx.send(self.service.current_plugin_lease_info(&plugin_id));
            }
            RuntimeActorMessage::PluginLeaseState { plugin_id, resp_tx } => {
                let _ = resp_tx.send(self.service.plugin_lease_state(&plugin_id));
            }
            RuntimeActorMessage::AcquireCurrentModuleLease { plugin_id, resp_tx } => {
                let _ = resp_tx.send(self.service.acquire_current_module_lease(&plugin_id));
            }
            RuntimeActorMessage::ActivePluginIds { resp_tx } => {
                let _ = resp_tx.send(self.service.active_plugin_ids());
            }
            RuntimeActorMessage::LoadDirAdditiveFromState { dir, resp_tx } => {
                let _ = resp_tx.send(self.service.load_dir_additive_from_state(&dir));
                should_refresh_introspection_cache = true;
            }
            RuntimeActorMessage::LoadDirAdditiveFiltered {
                dir,
                disabled_ids,
                resp_tx,
            } => {
                let _ = resp_tx.send(self.service.load_dir_additive_filtered(&dir, &disabled_ids));
                should_refresh_introspection_cache = true;
            }
            RuntimeActorMessage::ReloadDirFiltered {
                dir,
                disabled_ids,
                resp_tx,
            } => {
                let report = self.service.reload_dir_filtered(&dir, &disabled_ids);
                if let Ok(success) = &report {
                    self.emit_reload_notifications(success);
                }
                let _ = resp_tx.send(report);
                should_refresh_introspection_cache = true;
            }
            RuntimeActorMessage::ReloadDirFromState { dir, resp_tx } => {
                let report = self.service.reload_dir_from_state(&dir);
                if let Ok(success) = &report {
                    self.emit_reload_notifications(success);
                }
                let _ = resp_tx.send(report);
                should_refresh_introspection_cache = true;
            }
            RuntimeActorMessage::ReloadDirDetailedFromState { dir, resp_tx } => {
                let report = self.service.reload_dir_detailed_from_state(&dir);
                if let Ok(success) = &report {
                    self.emit_reload_notifications(&success.load_report);
                }
                let _ = resp_tx.send(report);
                should_refresh_introspection_cache = true;
            }
            RuntimeActorMessage::UnloadPlugin { plugin_id, resp_tx } => {
                let report = self.service.unload_plugin(&plugin_id);
                self.emit_worker_destroy(&plugin_id, "plugin unloaded");
                let _ = resp_tx.send(report);
                should_refresh_introspection_cache = true;
            }
            RuntimeActorMessage::ShutdownAndCleanup { resp_tx } => {
                let report = self.service.shutdown_and_cleanup();
                self.emit_worker_destroy_all("plugin runtime shutdown");
                let _ = resp_tx.send(report);
                should_refresh_introspection_cache = true;
            }
            RuntimeActorMessage::CleanupShadowCopiesNow { resp_tx } => {
                self.service.cleanup_shadow_copies_now();
                let _ = resp_tx.send(());
            }
            RuntimeActorMessage::CollectRetiredModuleLeasesByRefcount { resp_tx } => {
                let _ = resp_tx.send(self.service.collect_retired_module_leases_by_refcount());
            }
        }
        if should_refresh_introspection_cache {
            let refresh_started = Instant::now();
            self.refresh_introspection_cache_snapshot();
            let refresh_elapsed = refresh_started.elapsed();
            if refresh_elapsed.as_millis() > 50 {
                debug!(
                    elapsed_ms = refresh_elapsed.as_millis() as u64,
                    "introspection cache refresh was slow"
                );
            }
        }
        let total_elapsed = started.elapsed();
        debug!(
            message = msg,
            elapsed_ms = total_elapsed.as_millis() as u64,
            "plugin runtime actor handled message"
        );
    }
}

fn message_name(message: &RuntimeActorMessage) -> &'static str {
    match message {
        RuntimeActorMessage::RegisterWorkerControlSender { .. } => "RegisterWorkerControlSender",
        RuntimeActorMessage::SubscribeBackendControlRequests { .. } => {
            "SubscribeBackendControlRequests"
        }
        RuntimeActorMessage::SetDisabledPluginIds { .. } => "SetDisabledPluginIds",
        RuntimeActorMessage::PushHostEventJson { .. } => "PushHostEventJson",
        RuntimeActorMessage::BroadcastHostEventJson { .. } => "BroadcastHostEventJson",
        RuntimeActorMessage::SetPluginEnabled { .. } => "SetPluginEnabled",
        RuntimeActorMessage::DisabledPluginIds { .. } => "DisabledPluginIds",
        RuntimeActorMessage::ListActivePlugins { .. } => "ListActivePlugins",
        RuntimeActorMessage::CurrentModuleLeaseRef { .. } => "CurrentModuleLeaseRef",
        RuntimeActorMessage::CurrentPluginLeaseInfo { .. } => "CurrentPluginLeaseInfo",
        RuntimeActorMessage::PluginLeaseState { .. } => "PluginLeaseState",
        RuntimeActorMessage::AcquireCurrentModuleLease { .. } => "AcquireCurrentModuleLease",
        RuntimeActorMessage::ActivePluginIds { .. } => "ActivePluginIds",
        RuntimeActorMessage::LoadDirAdditiveFromState { .. } => "LoadDirAdditiveFromState",
        RuntimeActorMessage::LoadDirAdditiveFiltered { .. } => "LoadDirAdditiveFiltered",
        RuntimeActorMessage::ReloadDirFiltered { .. } => "ReloadDirFiltered",
        RuntimeActorMessage::ReloadDirFromState { .. } => "ReloadDirFromState",
        RuntimeActorMessage::ReloadDirDetailedFromState { .. } => "ReloadDirDetailedFromState",
        RuntimeActorMessage::UnloadPlugin { .. } => "UnloadPlugin",
        RuntimeActorMessage::ShutdownAndCleanup { .. } => "ShutdownAndCleanup",
        RuntimeActorMessage::CleanupShadowCopiesNow { .. } => "CleanupShadowCopiesNow",
        RuntimeActorMessage::CollectRetiredModuleLeasesByRefcount { .. } => {
            "CollectRetiredModuleLeasesByRefcount"
        }
    }
}

pub(crate) fn run_plugin_runtime_actor(
    rx: Receiver<RuntimeActorMessage>,
    host: StHostVTable,
    introspection_cache: Arc<ArcSwap<RuntimeIntrospectionReadCache>>,
) {
    let mut state = RuntimeActorState::new(host, introspection_cache);
    while let Ok(message) = rx.recv() {
        state.handle_message(message);
    }
}
