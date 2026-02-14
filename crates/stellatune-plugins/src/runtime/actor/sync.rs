use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use anyhow::{Result, anyhow};

use crate::manifest;
use crate::runtime::model::{
    ModuleLease, PluginSyncAction, RuntimeSyncActionOutcome, RuntimeSyncPlanSummary,
    RuntimeSyncReport, SourceLibraryFingerprint, SyncMode,
};

use super::PluginRuntimeActor;
use crate::load::{
    LoadedModuleCandidate, LoadedPluginModule, RuntimeLoadReport, RuntimePluginInfo,
    load_discovered_plugin,
};

impl PluginRuntimeActor {
    pub fn load_dir_additive_filtered(
        &mut self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        self.disabled_plugin_ids = disabled_ids.clone();
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
        self.disabled_plugin_ids = disabled_ids.clone();
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
        self.maybe_refresh_introspection_cache();
        report
    }

    pub fn shutdown_and_cleanup(&mut self) -> RuntimeLoadReport {
        let mut report = RuntimeLoadReport::default();
        let mut plugin_ids = self
            .modules
            .iter()
            .filter(|(_, slot)| slot.current.is_some())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect::<Vec<_>>();
        plugin_ids.sort();
        for plugin_id in plugin_ids {
            if self.disable_plugin_slot(&plugin_id) {
                report.deactivated.push(plugin_id.clone());
            }
            report.reclaimed_leases += self.gc_plugin_retired_leases(&plugin_id);
        }
        let _ = self.collect_retired_module_leases_by_refcount();
        self.cleanup_shadow_copies_best_effort("shutdown_and_cleanup");
        self.maybe_refresh_introspection_cache();
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

    pub(super) fn disable_plugin_slot(&mut self, plugin_id: &str) -> bool {
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
        self.mark_introspection_cache_dirty();
        true
    }

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
        let disabled_ids = self.disabled_plugin_ids.clone();
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
        self.maybe_refresh_introspection_cache();
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
        let active_ids = self
            .modules
            .iter()
            .filter(|(_, slot)| slot.current.is_some())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect::<HashSet<_>>();

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
        self.mark_introspection_cache_dirty();
    }

    fn reclaim_retired_module_leases_by_refcount(&mut self, plugin_id: &str) -> usize {
        let Some(slot) = self.modules.get_mut(plugin_id) else {
            return 0;
        };
        let mut reclaimed = 0usize;
        slot.retired.retain(|lease| {
            if std::sync::Arc::strong_count(lease) > 1 {
                true
            } else {
                reclaimed = reclaimed.saturating_add(1);
                false
            }
        });
        reclaimed
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
