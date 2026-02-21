use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::sync::Arc;

use crate::error::Result;
use parking_lot::RwLock;

use crate::executor::WasmPluginController;
use crate::manifest::discover_plugins;
use crate::runtime::directives::PluginDirectiveHub;
use crate::runtime::model::{
    DesiredPluginState, PluginDisableReason, RuntimePluginDirective, RuntimePluginLifecycleState,
    RuntimePluginTransition, RuntimePluginTransitionOutcome, RuntimePluginTransitionTrigger,
    RuntimeSyncReport,
};
use crate::runtime::registry::{
    ActivePlugin, RuntimeRegistry, active_plugin_from_manifest, build_plugin_statuses,
};

mod management;
mod query;

pub struct WasmPluginRuntime<C: WasmPluginController> {
    registry: Arc<RwLock<RuntimeRegistry>>,
    controller: Arc<C>,
    directives: Arc<PluginDirectiveHub>,
}

impl<C: WasmPluginController> Clone for WasmPluginRuntime<C> {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            controller: Arc::clone(&self.controller),
            directives: Arc::clone(&self.directives),
        }
    }
}

impl<C: WasmPluginController> WasmPluginRuntime<C> {
    pub fn new(controller: Arc<C>) -> Self {
        Self {
            registry: Arc::new(RwLock::new(RuntimeRegistry::default())),
            controller,
            directives: Arc::new(PluginDirectiveHub::default()),
        }
    }

    pub fn controller(&self) -> &C {
        self.controller.as_ref()
    }

    pub fn subscribe_plugin(
        &self,
        plugin_id: &str,
    ) -> Option<std::sync::mpsc::Receiver<RuntimePluginDirective>> {
        self.directives.subscribe_plugin(plugin_id)
    }

    pub fn notify_plugin(&self, plugin_id: &str, directive: RuntimePluginDirective) {
        let _ = self
            .controller
            .dispatch_directive(plugin_id, directive.clone());
        self.directives.notify_plugin(plugin_id, directive);
    }

    pub fn notify_plugin_update_config(&self, plugin_id: &str, config_json: String) {
        let directive = RuntimePluginDirective::UpdateConfig {
            config_json: config_json.clone(),
        };
        let _ = self
            .controller
            .dispatch_directive(plugin_id, directive.clone());
        self.directives.notify_plugin(plugin_id, directive);
    }

    pub fn sync_plugins(&self, dir: impl AsRef<Path>) -> Result<RuntimeSyncReport> {
        let dir = dir.as_ref();
        tracing::debug!(
            target: "stellatune_wasm_plugins::runtime",
            plugins_dir = %dir.display(),
            "plugin runtime sync begin"
        );
        let discovered = discover_plugins(dir)?;
        let discovered_plugins = discovered.len();

        let mut discovered_active = BTreeMap::<String, ActivePlugin>::new();
        let mut errors = Vec::<String>::new();
        let mut errors_by_plugin = BTreeMap::<String, String>::new();
        for item in discovered {
            let active = active_plugin_from_manifest(
                item.root_dir.clone(),
                item.manifest_path.clone(),
                item.manifest,
            );
            let plugin_id = active.info.id.clone();
            if discovered_active.contains_key(&plugin_id) {
                record_plugin_error(
                    &mut errors,
                    &mut errors_by_plugin,
                    &plugin_id,
                    format!("duplicate plugin id discovered during sync: {}", plugin_id),
                );
                continue;
            }
            discovered_active.insert(plugin_id, active);
        }

        let (previous_active, desired_states, previous_discovered_ids, previous_errors_by_plugin) = {
            let state = self.registry.read();
            (
                state.active_plugins.clone(),
                state.desired_states.clone(),
                state.last_discovered_plugin_ids.clone(),
                state.last_errors_by_plugin.clone(),
            )
        };

        // Start from the previous runtime view and mutate by transition results.
        // This avoids dropping active entries when uninstall fails.
        let mut next_active = previous_active.clone();
        let mut transitions = Vec::<RuntimePluginTransition>::new();
        let mut deactivated = HashSet::<String>::new();

        for (plugin_id, discovered_plugin) in &discovered_active {
            let previous_lifecycle = lifecycle_state_from_snapshot(
                plugin_id,
                &previous_active,
                &previous_discovered_ids,
                &desired_states,
                &previous_errors_by_plugin,
            );
            let desired_state = desired_states
                .get(plugin_id)
                .copied()
                .unwrap_or(DesiredPluginState::Enabled);
            if desired_state == DesiredPluginState::Disabled {
                let was_active = previous_active.contains_key(plugin_id);
                if was_active
                    && !try_uninstall_plugin(
                        self.controller.as_ref(),
                        plugin_id,
                        PluginDisableReason::HostDisable,
                        &mut deactivated,
                        &mut errors,
                        &mut errors_by_plugin,
                    )
                {
                    transitions.push(RuntimePluginTransition {
                        plugin_id: plugin_id.clone(),
                        from: previous_lifecycle,
                        to: RuntimePluginLifecycleState::Failed,
                        trigger: RuntimePluginTransitionTrigger::DisableRequested,
                        outcome: RuntimePluginTransitionOutcome::Failed,
                        detail: "uninstall_failed".to_string(),
                    });
                    continue;
                }
                if was_active {
                    next_active.remove(plugin_id);
                    errors_by_plugin.remove(plugin_id);
                    transitions.push(RuntimePluginTransition {
                        plugin_id: plugin_id.clone(),
                        from: previous_lifecycle,
                        to: RuntimePluginLifecycleState::Disabled,
                        trigger: RuntimePluginTransitionTrigger::DisableRequested,
                        outcome: RuntimePluginTransitionOutcome::Applied,
                        detail: "disabled".to_string(),
                    });
                } else {
                    next_active.remove(plugin_id);
                    transitions.push(RuntimePluginTransition {
                        plugin_id: plugin_id.clone(),
                        from: previous_lifecycle,
                        to: RuntimePluginLifecycleState::Disabled,
                        trigger: RuntimePluginTransitionTrigger::DisableRequested,
                        outcome: RuntimePluginTransitionOutcome::Skipped,
                        detail: "already_inactive".to_string(),
                    });
                }
                continue;
            }

            let previous = previous_active.get(plugin_id);
            let needs_activation =
                previous.is_none_or(|prev| prev.signature != discovered_plugin.signature);
            if !needs_activation {
                if previous.is_some() {
                    next_active.insert(plugin_id.clone(), discovered_plugin.clone());
                }
                errors_by_plugin.remove(plugin_id);
                continue;
            }

            let trigger = if previous.is_some() {
                RuntimePluginTransitionTrigger::ReloadChanged
            } else {
                RuntimePluginTransitionTrigger::LoadNew
            };

            if previous.is_some()
                && !try_uninstall_plugin(
                    self.controller.as_ref(),
                    plugin_id,
                    PluginDisableReason::Reload,
                    &mut deactivated,
                    &mut errors,
                    &mut errors_by_plugin,
                )
            {
                transitions.push(RuntimePluginTransition {
                    plugin_id: plugin_id.clone(),
                    from: previous_lifecycle,
                    to: RuntimePluginLifecycleState::Failed,
                    trigger,
                    outcome: RuntimePluginTransitionOutcome::Failed,
                    detail: "uninstall_failed".to_string(),
                });
                continue;
            }
            if previous.is_some() {
                next_active.remove(plugin_id);
            }

            if let Err(error) = self
                .controller
                .install_plugin(&discovered_plugin.info, &discovered_plugin.capabilities)
            {
                record_plugin_error(
                    &mut errors,
                    &mut errors_by_plugin,
                    plugin_id,
                    format!("install plugin `{}` failed: {:#}", plugin_id, error),
                );
                transitions.push(RuntimePluginTransition {
                    plugin_id: plugin_id.clone(),
                    from: previous_lifecycle,
                    to: RuntimePluginLifecycleState::Failed,
                    trigger,
                    outcome: RuntimePluginTransitionOutcome::Failed,
                    detail: "install_failed".to_string(),
                });
                continue;
            }

            next_active.insert(plugin_id.clone(), discovered_plugin.clone());
            errors_by_plugin.remove(plugin_id);
            transitions.push(RuntimePluginTransition {
                plugin_id: plugin_id.clone(),
                from: previous_lifecycle,
                to: RuntimePluginLifecycleState::Active,
                trigger,
                outcome: RuntimePluginTransitionOutcome::Applied,
                detail: if trigger == RuntimePluginTransitionTrigger::ReloadChanged {
                    "manifest_changed".to_string()
                } else {
                    "new".to_string()
                },
            });
        }

        for plugin_id in previous_active.keys() {
            if !next_active.contains_key(plugin_id) || discovered_active.contains_key(plugin_id) {
                continue;
            }
            let previous_lifecycle = lifecycle_state_from_snapshot(
                plugin_id,
                &previous_active,
                &previous_discovered_ids,
                &desired_states,
                &previous_errors_by_plugin,
            );
            if !try_uninstall_plugin(
                self.controller.as_ref(),
                plugin_id,
                PluginDisableReason::Unload,
                &mut deactivated,
                &mut errors,
                &mut errors_by_plugin,
            ) {
                transitions.push(RuntimePluginTransition {
                    plugin_id: plugin_id.clone(),
                    from: previous_lifecycle,
                    to: RuntimePluginLifecycleState::Failed,
                    trigger: RuntimePluginTransitionTrigger::RemovedFromDisk,
                    outcome: RuntimePluginTransitionOutcome::Failed,
                    detail: "uninstall_failed".to_string(),
                });
                continue;
            }
            next_active.remove(plugin_id);
            errors_by_plugin.remove(plugin_id);
            transitions.push(RuntimePluginTransition {
                plugin_id: plugin_id.clone(),
                from: previous_lifecycle,
                to: RuntimePluginLifecycleState::Missing,
                trigger: RuntimePluginTransitionTrigger::RemovedFromDisk,
                outcome: RuntimePluginTransitionOutcome::Applied,
                detail: "removed".to_string(),
            });
        }

        let discovered_ids = discovered_active.keys().cloned().collect::<BTreeSet<_>>();
        let mut state = self.registry.write();
        state.revision = state.revision.saturating_add(1);
        let revision = state.revision;
        state.active_plugins = next_active;
        state.last_discovered_plugin_ids = discovered_ids.clone();
        state.last_errors_by_plugin = errors_by_plugin.clone();

        let mut active_plugins = state
            .active_plugins
            .values()
            .map(|plugin| plugin.info.clone())
            .collect::<Vec<_>>();
        active_plugins.sort_by(|a, b| a.id.cmp(&b.id));

        let plugin_statuses = build_plugin_statuses(
            &state.active_plugins,
            &discovered_ids,
            &state.desired_states,
            &state.last_errors_by_plugin,
        );

        transitions.sort_by(|a, b| {
            a.plugin_id
                .cmp(&b.plugin_id)
                .then_with(|| a.detail.cmp(&b.detail))
        });

        for transition in &transitions {
            tracing::debug!(
                target: "stellatune_wasm_plugins::runtime",
                plugin_id = %transition.plugin_id,
                from = ?transition.from,
                to = ?transition.to,
                trigger = ?transition.trigger,
                outcome = ?transition.outcome,
                detail = %transition.detail,
                "plugin runtime transition"
            );
            if transition.outcome != RuntimePluginTransitionOutcome::Applied {
                continue;
            }
            match transition.to {
                RuntimePluginLifecycleState::Active => {
                    self.notify_plugin(&transition.plugin_id, RuntimePluginDirective::Rebuild);
                },
                RuntimePluginLifecycleState::Disabled | RuntimePluginLifecycleState::Missing => {
                    let reason = match transition.trigger {
                        RuntimePluginTransitionTrigger::RemovedFromDisk => {
                            PluginDisableReason::Unload
                        },
                        _ => PluginDisableReason::HostDisable,
                    };
                    self.notify_plugin(
                        &transition.plugin_id,
                        RuntimePluginDirective::Destroy { reason },
                    );
                },
                RuntimePluginLifecycleState::Failed => {},
            }
        }

        let failed_transitions = transitions
            .iter()
            .filter(|item| item.outcome == RuntimePluginTransitionOutcome::Failed)
            .count();
        let disabled_statuses = plugin_statuses
            .iter()
            .filter(|item| item.lifecycle_state == RuntimePluginLifecycleState::Disabled)
            .count();
        let failed_statuses = plugin_statuses
            .iter()
            .filter(|item| item.lifecycle_state == RuntimePluginLifecycleState::Failed)
            .count();
        let missing_statuses = plugin_statuses
            .iter()
            .filter(|item| item.lifecycle_state == RuntimePluginLifecycleState::Missing)
            .count();
        let mut active_ids = active_plugins
            .iter()
            .map(|plugin| plugin.id.clone())
            .collect::<Vec<_>>();
        active_ids.sort();
        tracing::info!(
            target: "stellatune_wasm_plugins::runtime",
            revision,
            plugins_dir = %dir.display(),
            discovered = discovered_plugins,
            active = active_ids.len(),
            transitions = transitions.len(),
            transition_failed = failed_transitions,
            status_disabled = disabled_statuses,
            status_failed = failed_statuses,
            status_missing = missing_statuses,
            errors = errors.len(),
            active_plugins = ?active_ids,
            "plugin runtime sync completed"
        );
        for status in plugin_statuses
            .iter()
            .filter(|item| item.last_error.is_some())
        {
            tracing::warn!(
                target: "stellatune_wasm_plugins::runtime",
                plugin_id = %status.plugin_id,
                desired = ?status.desired_state,
                lifecycle = ?status.lifecycle_state,
                last_error = %status.last_error.as_deref().unwrap_or_default(),
                "plugin runtime status carries error"
            );
        }

        Ok(RuntimeSyncReport {
            revision,
            discovered_plugins,
            active_plugins,
            plugin_statuses,
            transitions,
            errors,
        })
    }
}

fn try_uninstall_plugin<C: WasmPluginController>(
    controller: &C,
    plugin_id: &str,
    reason: PluginDisableReason,
    deactivated: &mut HashSet<String>,
    errors: &mut Vec<String>,
    errors_by_plugin: &mut BTreeMap<String, String>,
) -> bool {
    if deactivated.contains(plugin_id) {
        return true;
    }
    match controller.uninstall_plugin(plugin_id, reason) {
        Ok(()) => {
            deactivated.insert(plugin_id.to_string());
            true
        },
        Err(error) => {
            record_plugin_error(
                errors,
                errors_by_plugin,
                plugin_id,
                format!("uninstall plugin `{}` failed: {:#}", plugin_id, error),
            );
            false
        },
    }
}

fn record_plugin_error(
    errors: &mut Vec<String>,
    errors_by_plugin: &mut BTreeMap<String, String>,
    plugin_id: &str,
    message: String,
) {
    tracing::warn!(
        target: "stellatune_wasm_plugins::runtime",
        plugin_id = %plugin_id,
        error = %message,
        "plugin runtime sync error"
    );
    errors.push(message.clone());
    errors_by_plugin
        .entry(plugin_id.to_string())
        .and_modify(|existing| {
            if !existing.is_empty() {
                existing.push_str("; ");
            }
            existing.push_str(&message);
        })
        .or_insert(message);
}

fn lifecycle_state_from_snapshot(
    plugin_id: &str,
    active_plugins: &BTreeMap<String, ActivePlugin>,
    discovered_ids: &BTreeSet<String>,
    desired_states: &BTreeMap<String, DesiredPluginState>,
    errors_by_plugin: &BTreeMap<String, String>,
) -> RuntimePluginLifecycleState {
    let desired_state = desired_states
        .get(plugin_id)
        .copied()
        .unwrap_or(DesiredPluginState::Enabled);
    if active_plugins.contains_key(plugin_id) {
        return RuntimePluginLifecycleState::Active;
    }
    if !discovered_ids.contains(plugin_id) {
        return RuntimePluginLifecycleState::Missing;
    }
    if desired_state == DesiredPluginState::Disabled {
        return RuntimePluginLifecycleState::Disabled;
    }
    if errors_by_plugin.contains_key(plugin_id) {
        return RuntimePluginLifecycleState::Failed;
    }
    RuntimePluginLifecycleState::Failed
}
