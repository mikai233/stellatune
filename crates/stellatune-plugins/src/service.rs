use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use crossbeam_channel::Receiver;
use stellatune_plugin_api::{STELLATUNE_PLUGIN_API_VERSION, StHostVTable};
use stellatune_plugin_api::{StLogLevel, StStr};

use crate::events::{PluginEventBus, new_runtime_event_bus};
use crate::manifest;
use crate::runtime::backend_control::BackendControlRequest;
use crate::runtime::model::{
    ModuleLease, ModuleLeaseRef, PluginSyncAction, RuntimeSyncActionOutcome,
    RuntimeSyncPlanSummary, RuntimeSyncReport, SourceLibraryFingerprint, SyncMode,
};
use crate::runtime::registry::PluginModuleLeaseSlotState;

use super::load::{
    LoadedModuleCandidate, LoadedPluginModule, RuntimeLoadReport, RuntimePluginInfo,
    cleanup_stale_shadow_libraries, load_discovered_plugin,
};

const SHADOW_CLEANUP_GRACE_PERIOD: Duration = Duration::ZERO;
const SHADOW_CLEANUP_MAX_DELETIONS_PER_RUN: usize = 200;

pub struct PluginRuntimeService {
    host: StHostVTable,
    event_bus: PluginEventBus,
    modules: HashMap<String, PluginModuleLeaseSlotState>,
    disabled_plugin_ids: HashSet<String>,
}

impl PluginRuntimeService {
    pub fn new(host: StHostVTable) -> Self {
        Self {
            host,
            event_bus: new_runtime_event_bus(),
            modules: HashMap::new(),
            disabled_plugin_ids: HashSet::new(),
        }
    }

    pub fn push_host_event_json(&self, plugin_id: &str, event_json: &str) {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() || event_json.is_empty() {
            return;
        }
        self.event_bus
            .push_host_event(plugin_id, event_json.to_string());
    }

    pub fn broadcast_host_event_json(&self, event_json: &str) {
        if event_json.is_empty() {
            return;
        }
        for plugin_id in self.event_bus.registered_plugin_ids() {
            self.event_bus
                .push_host_event(&plugin_id, event_json.to_string());
        }
    }

    pub fn subscribe_backend_control_requests(&self) -> Receiver<BackendControlRequest> {
        self.event_bus.subscribe_control_requests()
    }

    pub fn set_disabled_plugin_ids(&mut self, disabled_ids: HashSet<String>) {
        self.disabled_plugin_ids = disabled_ids;
    }

    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return;
        }
        if enabled {
            self.disabled_plugin_ids.remove(plugin_id);
        } else {
            self.disabled_plugin_ids.insert(plugin_id.to_string());
        }
    }

    pub fn disabled_plugin_ids(&self) -> HashSet<String> {
        self.disabled_plugin_ids.clone()
    }

    pub fn list_active_plugins(&self) -> Vec<RuntimePluginInfo> {
        let mut plugin_ids = self.active_plugin_ids();
        plugin_ids.sort();
        let mut out = Vec::with_capacity(plugin_ids.len());
        for plugin_id in plugin_ids {
            let Some(slot) = self.modules.get(&plugin_id) else {
                continue;
            };
            let Some(current) = slot.current.as_ref() else {
                continue;
            };
            let mut info = RuntimePluginInfo {
                id: plugin_id.clone(),
                name: plugin_id.clone(),
                metadata_json: current.metadata_json.clone(),
                root_dir: None,
                library_path: None,
            };
            info.name = current.plugin_name.clone();
            info.root_dir = Some(current.loaded.root_dir.clone());
            info.library_path = Some(current.loaded.library_path.clone());
            out.push(info);
        }
        out
    }

    pub fn current_module_lease_ref(&self, plugin_id: &str) -> Option<ModuleLeaseRef> {
        let slot = self.modules.get(plugin_id)?;
        let lease = slot.current.as_ref()?;
        Some(ModuleLeaseRef::from_arc(lease))
    }

    pub(crate) fn acquire_current_module_lease(&self, plugin_id: &str) -> Option<Arc<ModuleLease>> {
        if self.disabled_plugin_ids.contains(plugin_id) {
            return None;
        }
        let slot = self.modules.get(plugin_id)?;
        slot.current.as_ref().cloned()
    }

    pub fn active_plugin_ids(&self) -> Vec<String> {
        self.modules
            .iter()
            .filter(|(_, slot)| slot.current.is_some())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect()
    }

    fn disable_plugin_slot(&mut self, plugin_id: &str) -> bool {
        let retired = {
            let Some(slot) = self.modules.get_mut(plugin_id) else {
                return false;
            };
            slot.retire_current()
        };
        if !retired {
            return false;
        }
        tracing::debug!(plugin_id, "plugin lease deactivated");
        true
    }

    /// Collect and unload retired plugin module leases.
    fn gc_plugin_retired_leases(&mut self, plugin_id: &str) -> usize {
        if !self.modules.contains_key(plugin_id) {
            tracing::debug!(
                plugin_id,
                "skip retired lease gc because module lease slot is missing"
            );
            return 0;
        }

        let reclaimed = self.reclaim_retired_module_leases_by_refcount(plugin_id);
        if reclaimed == 0 {
            return 0;
        }

        let remove_module_slot = self
            .modules
            .get(plugin_id)
            .map(|slot| slot.current.is_none() && slot.retired.is_empty())
            .unwrap_or(false);
        if remove_module_slot {
            self.modules.remove(plugin_id);
        }
        tracing::debug!(
            plugin_id,
            reclaimed_leases = reclaimed,
            "plugin retired leases reclaimed by refcount"
        );
        self.cleanup_shadow_copies_best_effort("collect_ready_for_unload");
        reclaimed
    }

    pub fn load_dir_additive_filtered(
        &mut self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        self.set_disabled_plugin_ids(disabled_ids.clone());
        self.load_dir_additive_from_state(dir)
    }

    pub fn load_dir_additive_from_state(
        &mut self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeLoadReport> {
        self.sync_dir_from_state_report(dir, SyncMode::Additive)
            .map(|report| report.load_report)
    }

    pub fn reload_dir_filtered(
        &mut self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        self.set_disabled_plugin_ids(disabled_ids.clone());
        self.reload_dir_from_state(dir)
    }

    pub fn reload_dir_from_state(&mut self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        self.sync_dir_from_state_report(dir, SyncMode::Reconcile)
            .map(|report| report.load_report)
    }

    pub fn reload_dir_detailed_from_state(
        &mut self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        self.sync_dir_from_state_report(dir, SyncMode::Reconcile)
    }

    pub fn unload_plugin(&mut self, plugin_id: &str) -> RuntimeLoadReport {
        let mut report = RuntimeLoadReport::default();
        if self.disable_plugin_slot(plugin_id) {
            report.deactivated.push(plugin_id.to_string());
        }
        report.reclaimed_leases += self.gc_plugin_retired_leases(plugin_id);
        let _ = self.collect_retired_module_leases_by_refcount();
        self.cleanup_shadow_copies_best_effort("unload_plugin:end");
        report
    }

    pub fn shutdown_and_cleanup(&mut self) -> RuntimeLoadReport {
        let mut report = RuntimeLoadReport::default();
        let mut plugin_ids = self.active_plugin_ids();
        plugin_ids.sort();
        for plugin_id in plugin_ids {
            if self.disable_plugin_slot(&plugin_id) {
                report.deactivated.push(plugin_id.clone());
            }
            report.reclaimed_leases += self.gc_plugin_retired_leases(&plugin_id);
        }
        let _ = self.collect_retired_module_leases_by_refcount();
        self.cleanup_shadow_copies_best_effort("shutdown_and_cleanup");
        report
    }

    pub fn collect_retired_module_leases_by_refcount(&mut self) -> usize {
        let plugin_ids = self.modules.keys().cloned().collect::<Vec<_>>();
        let mut reclaimed = 0usize;
        for plugin_id in plugin_ids {
            reclaimed = reclaimed
                .saturating_add(self.reclaim_retired_module_leases_by_refcount(&plugin_id));
            let remove = self
                .modules
                .get(&plugin_id)
                .map(|slot| slot.current.is_none() && slot.retired.is_empty())
                .unwrap_or(false);
            if remove {
                self.modules.remove(&plugin_id);
            }
        }
        if reclaimed > 0 {
            tracing::debug!(
                reclaimed_retired_leases = reclaimed,
                "plugin retired leases reclaimed by refcount"
            );
            self.cleanup_shadow_copies_best_effort("collect_retired_module_leases_by_refcount");
        }
        reclaimed
    }

    pub fn cleanup_shadow_copies_now(&self) {
        self.cleanup_shadow_copies_best_effort("cleanup_shadow_copies_now");
    }

    fn sync_dir_from_state_report(
        &mut self,
        dir: impl AsRef<Path>,
        mode: SyncMode,
    ) -> Result<RuntimeSyncReport> {
        let total_started = Instant::now();
        let (begin_reason, end_reason) = match mode {
            SyncMode::Additive => (
                "sync_dir_from_state_additive:begin",
                "sync_dir_from_state_additive:end",
            ),
            SyncMode::Reconcile => (
                "sync_dir_from_state_reconcile:begin",
                "sync_dir_from_state_reconcile:end",
            ),
        };
        self.cleanup_shadow_copies_best_effort(begin_reason);

        let dir = dir.as_ref();
        let plan_started = Instant::now();
        let disabled_ids = self.disabled_plugin_ids();
        let discovered_plugins = manifest::discover_plugins(dir)?;
        let plan = self.plan_sync_actions(&discovered_plugins, &disabled_ids, mode);
        let mut plan_summary = RuntimeSyncPlanSummary {
            discovered: discovered_plugins.len(),
            disabled: disabled_ids.len(),
            actions_total: plan.len(),
            ..RuntimeSyncPlanSummary::default()
        };
        for action in &plan {
            match action {
                PluginSyncAction::LoadNew { .. } => plan_summary.load_new += 1,
                PluginSyncAction::ReloadChanged { .. } => plan_summary.reload_changed += 1,
                PluginSyncAction::DeactivateMissingOrDisabled { .. } => {
                    plan_summary.deactivate += 1
                }
            }
        }
        let plan_ms = plan_started.elapsed().as_millis() as u64;
        tracing::debug!(
            mode = ?mode,
            discovered = plan_summary.discovered,
            disabled = plan_summary.disabled,
            actions = plan_summary.actions_total,
            load_new = plan_summary.load_new,
            reload_changed = plan_summary.reload_changed,
            deactivate = plan_summary.deactivate,
            "plugin sync plan prepared"
        );
        let discovered_by_id = discovered_plugins
            .iter()
            .map(|plugin| (plugin.manifest.id.clone(), plugin))
            .collect::<HashMap<_, _>>();

        let execute_started = Instant::now();
        let mut report = RuntimeLoadReport::default();
        let mut action_outcomes = Vec::new();
        for action in plan {
            match action {
                PluginSyncAction::LoadNew { plugin_id } => {
                    let Some(discovered) = discovered_by_id.get(&plugin_id) else {
                        report.errors.push(anyhow!(
                            "planner inconsistency: missing discovered plugin `{plugin_id}`"
                        ));
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "load_new".to_string(),
                            plugin_id,
                            outcome: "planner_missing_discovered".to_string(),
                        });
                        continue;
                    };
                    match load_discovered_plugin(discovered, &self.host, self.event_bus.clone()) {
                        Ok(candidate) => {
                            let activated = self.activate_loaded_candidate(candidate);
                            report.reclaimed_leases += activated.reclaimed_leases;
                            report.loaded.push(activated.info);
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "load_new".to_string(),
                                plugin_id,
                                outcome: "loaded".to_string(),
                            });
                        }
                        Err(error) => {
                            report
                                .errors
                                .push(error.context(format!("while loading plugin `{plugin_id}`")));
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "load_new".to_string(),
                                plugin_id,
                                outcome: "error".to_string(),
                            });
                        }
                    }
                }
                PluginSyncAction::ReloadChanged { plugin_id } => {
                    let Some(discovered) = discovered_by_id.get(&plugin_id) else {
                        report.errors.push(anyhow!(
                            "planner inconsistency: missing discovered plugin `{plugin_id}`"
                        ));
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "reload_changed".to_string(),
                            plugin_id,
                            outcome: "planner_missing_discovered".to_string(),
                        });
                        continue;
                    };
                    match load_discovered_plugin(discovered, &self.host, self.event_bus.clone()) {
                        Ok(candidate) => {
                            let activated = self.activate_loaded_candidate(candidate);
                            report.reclaimed_leases += activated.reclaimed_leases;
                            report.loaded.push(activated.info);
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "reload_changed".to_string(),
                                plugin_id,
                                outcome: "reloaded".to_string(),
                            });
                        }
                        Err(error) => {
                            report.errors.push(
                                error.context(format!(
                                    "while reloading changed plugin `{plugin_id}`"
                                )),
                            );
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "reload_changed".to_string(),
                                plugin_id,
                                outcome: "error".to_string(),
                            });
                        }
                    }
                }
                PluginSyncAction::DeactivateMissingOrDisabled { plugin_id } => {
                    if self.disable_plugin_slot(&plugin_id) {
                        report.deactivated.push(plugin_id.clone());
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "deactivate".to_string(),
                            plugin_id: plugin_id.clone(),
                            outcome: "deactivated".to_string(),
                        });
                    } else {
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "deactivate".to_string(),
                            plugin_id: plugin_id.clone(),
                            outcome: "already_inactive".to_string(),
                        });
                    }
                    report.reclaimed_leases += self.gc_plugin_retired_leases(&plugin_id);
                }
            }
        }
        let execute_ms = execute_started.elapsed().as_millis() as u64;

        let _ = self.collect_retired_module_leases_by_refcount();
        self.cleanup_shadow_copies_best_effort(end_reason);
        let total_ms = total_started.elapsed().as_millis() as u64;
        Ok(RuntimeSyncReport {
            load_report: report,
            plan: plan_summary,
            actions: action_outcomes,
            plan_ms,
            execute_ms,
            total_ms,
        })
    }

    fn plan_sync_actions(
        &self,
        discovered_plugins: &[manifest::DiscoveredPlugin],
        disabled_ids: &HashSet<String>,
        mode: SyncMode,
    ) -> Vec<PluginSyncAction> {
        let discovered_ids = discovered_plugins
            .iter()
            .map(|plugin| plugin.manifest.id.clone())
            .collect::<HashSet<_>>();
        let active_ids = self.active_plugin_ids().into_iter().collect::<HashSet<_>>();

        let mut actions = Vec::new();
        for plugin in discovered_plugins {
            let plugin_id = plugin.manifest.id.trim();
            if plugin_id.is_empty() {
                continue;
            }
            let plugin_id = plugin_id.to_string();
            if disabled_ids.contains(&plugin_id) {
                if matches!(mode, SyncMode::Reconcile) && active_ids.contains(&plugin_id) {
                    actions.push(PluginSyncAction::DeactivateMissingOrDisabled { plugin_id });
                }
                continue;
            }

            match mode {
                SyncMode::Additive => {
                    if !active_ids.contains(&plugin_id) {
                        actions.push(PluginSyncAction::LoadNew { plugin_id });
                    }
                }
                SyncMode::Reconcile => {
                    if !active_ids.contains(&plugin_id) {
                        actions.push(PluginSyncAction::LoadNew { plugin_id });
                        continue;
                    }
                    let next_fingerprint = source_fingerprint_for_path(&plugin.library_path);
                    let active_fingerprint = self.active_source_fingerprint(&plugin_id);
                    if active_fingerprint != Some(next_fingerprint) {
                        actions.push(PluginSyncAction::ReloadChanged { plugin_id });
                    }
                }
            }
        }

        if matches!(mode, SyncMode::Reconcile) {
            for plugin_id in active_ids {
                if disabled_ids.contains(&plugin_id) || !discovered_ids.contains(&plugin_id) {
                    actions.push(PluginSyncAction::DeactivateMissingOrDisabled { plugin_id });
                }
            }
        }

        actions
    }

    fn active_source_fingerprint(&self, plugin_id: &str) -> Option<SourceLibraryFingerprint> {
        let slot = self.modules.get(plugin_id)?;
        slot.current
            .as_ref()
            .map(|current| current.source_fingerprint.clone())
    }

    fn activate_loaded_candidate(&mut self, candidate: LoadedModuleCandidate) -> ActivatedLoad {
        self.activate_loaded_module(
            &candidate.plugin_id,
            candidate.plugin_name.clone(),
            candidate.metadata_json.clone(),
            candidate.loaded_module,
        );
        let reclaimed_leases = self.gc_plugin_retired_leases(&candidate.plugin_id);
        ActivatedLoad {
            info: RuntimePluginInfo {
                id: candidate.plugin_id,
                name: candidate.plugin_name,
                metadata_json: candidate.metadata_json,
                root_dir: Some(candidate.root_dir),
                library_path: Some(candidate.library_path),
            },
            reclaimed_leases,
        }
    }

    fn activate_loaded_module(
        &mut self,
        plugin_id: &str,
        plugin_name: String,
        metadata_json: String,
        loaded: LoadedPluginModule,
    ) {
        let source_fingerprint = source_fingerprint_for_path(&loaded.library_path);
        self.modules
            .entry(plugin_id.to_string())
            .or_default()
            .set_current(ModuleLease {
                plugin_id: plugin_id.to_string(),
                plugin_name,
                metadata_json,
                source_fingerprint,
                loaded,
            });
    }

    fn reclaim_retired_module_leases_by_refcount(&mut self, plugin_id: &str) -> usize {
        let Some(slot) = self.modules.get_mut(plugin_id) else {
            return 0;
        };
        let mut reclaimed = 0usize;
        slot.retired.retain(|lease| {
            if Arc::strong_count(lease) > 1 {
                true
            } else {
                reclaimed = reclaimed.saturating_add(1);
                false
            }
        });
        reclaimed
    }

    fn collect_protected_shadow_paths(&self) -> HashSet<std::path::PathBuf> {
        let mut out = HashSet::new();
        for slot in self.modules.values() {
            if let Some(current) = slot.current.as_ref() {
                out.insert(current.loaded.shadow_library_path.clone());
            }
            for retired in &slot.retired {
                out.insert(retired.loaded.shadow_library_path.clone());
            }
        }
        out
    }

    fn cleanup_shadow_copies_best_effort(&self, reason: &str) {
        let protected = self.collect_protected_shadow_paths();
        let report = cleanup_stale_shadow_libraries(
            &protected,
            SHADOW_CLEANUP_GRACE_PERIOD,
            SHADOW_CLEANUP_MAX_DELETIONS_PER_RUN,
        );
        if report.scanned == 0
            && report.deleted == 0
            && report.failed == 0
            && report.skipped_active == 0
            && report.skipped_recent_current_process == 0
            && report.skipped_unrecognized == 0
        {
            return;
        }
        tracing::debug!(
            reason,
            plugin_shadow_scanned = report.scanned,
            plugin_shadow_deleted = report.deleted,
            plugin_shadow_failed = report.failed,
            plugin_shadow_skipped_active = report.skipped_active,
            plugin_shadow_skipped_recent_current_process = report.skipped_recent_current_process,
            plugin_shadow_skipped_unrecognized = report.skipped_unrecognized,
            "plugin shadow cleanup completed"
        );
    }
}

struct ActivatedLoad {
    info: RuntimePluginInfo,
    reclaimed_leases: usize,
}

fn source_fingerprint_for_path(path: &Path) -> SourceLibraryFingerprint {
    let mut file_size = 0;
    let mut modified_unix_ms = 0;
    if let Ok(meta) = std::fs::metadata(path) {
        file_size = meta.len();
        if let Ok(modified) = meta.modified() {
            modified_unix_ms = modified
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
        }
    }
    SourceLibraryFingerprint {
        library_path: path.to_path_buf(),
        file_size,
        modified_unix_ms,
    }
}

pub(crate) fn default_host_vtable() -> StHostVTable {
    extern "C" fn default_host_log(_: *mut core::ffi::c_void, level: StLogLevel, msg: StStr) {
        let text = unsafe { crate::util::ststr_to_string_lossy(msg) };
        match level {
            StLogLevel::Error => tracing::error!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Warn => tracing::warn!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Info => tracing::info!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Debug => tracing::debug!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Trace => tracing::trace!(target: "stellatune_plugins::plugin", "{text}"),
        }
    }

    StHostVTable {
        api_version: STELLATUNE_PLUGIN_API_VERSION,
        user_data: core::ptr::null_mut(),
        log_utf8: Some(default_host_log),
        get_runtime_root_utf8: None,
        emit_event_json_utf8: None,
        poll_host_event_json_utf8: None,
        send_control_json_utf8: None,
        free_host_str_utf8: None,
    }
}

#[cfg(test)]
#[path = "tests/service_tests.rs"]
mod tests;
