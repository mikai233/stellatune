use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::error::Result;

use crate::executor::WasmPluginController;
use crate::manifest::discover_plugins;
use crate::runtime::directives::PluginDirectiveHub;
use crate::runtime::model::{
    DesiredPluginState, PluginDisableReason, RuntimePluginChange, RuntimePluginChangeKind,
    RuntimePluginDirective, RuntimeSyncReport,
};
use crate::runtime::registry::{
    ActivePlugin, RuntimeRegistry, active_plugin_from_manifest, build_plugin_statuses,
};

mod management;
mod query;

#[derive(Clone)]
pub struct WasmPluginRuntime {
    registry: Arc<RwLock<RuntimeRegistry>>,
    controller: Arc<dyn WasmPluginController>,
    directives: Arc<PluginDirectiveHub>,
}

impl WasmPluginRuntime {
    pub fn new(controller: Arc<dyn WasmPluginController>) -> Self {
        Self {
            registry: Arc::new(RwLock::new(RuntimeRegistry::default())),
            controller,
            directives: Arc::new(PluginDirectiveHub::default()),
        }
    }

    pub fn with_controller(controller: Arc<dyn WasmPluginController>) -> Self {
        Self::new(controller)
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
        let discovered = discover_plugins(dir)?;
        let discovered_plugins = discovered.len();

        let mut discovered_active = BTreeMap::<String, ActivePlugin>::new();
        let mut errors = Vec::<String>::new();
        for item in discovered {
            let active = active_plugin_from_manifest(
                item.root_dir.clone(),
                item.manifest_path.clone(),
                item.manifest,
            );
            let plugin_id = active.info.id.clone();
            if discovered_active.contains_key(&plugin_id) {
                errors.push(format!(
                    "duplicate plugin id discovered during sync: {}",
                    plugin_id
                ));
                continue;
            }
            discovered_active.insert(plugin_id, active);
        }

        let (previous_active, desired_states) = {
            let state = self
                .registry
                .read()
                .expect("runtime registry lock poisoned");
            (state.active_plugins.clone(), state.desired_states.clone())
        };

        let mut next_active = BTreeMap::<String, ActivePlugin>::new();
        let mut changes = Vec::<RuntimePluginChange>::new();
        let mut deactivated = HashSet::<String>::new();

        for (plugin_id, discovered_plugin) in &discovered_active {
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
                    )
                {
                    changes.push(RuntimePluginChange {
                        plugin_id: plugin_id.clone(),
                        kind: RuntimePluginChangeKind::Skipped,
                        reason: "uninstall_failed".to_string(),
                    });
                    continue;
                }
                changes.push(RuntimePluginChange {
                    plugin_id: plugin_id.clone(),
                    kind: if was_active {
                        RuntimePluginChangeKind::Deactivated
                    } else {
                        RuntimePluginChangeKind::Skipped
                    },
                    reason: "disabled".to_string(),
                });
                continue;
            }

            let previous = previous_active.get(plugin_id);
            let needs_activation =
                previous.is_none_or(|prev| prev.signature != discovered_plugin.signature);
            if needs_activation {
                let change_kind = if previous.is_some() {
                    RuntimePluginChangeKind::Reloaded
                } else {
                    RuntimePluginChangeKind::Activated
                };

                if previous.is_some()
                    && !try_uninstall_plugin(
                        self.controller.as_ref(),
                        plugin_id,
                        PluginDisableReason::Reload,
                        &mut deactivated,
                        &mut errors,
                    )
                {
                    changes.push(RuntimePluginChange {
                        plugin_id: plugin_id.clone(),
                        kind: RuntimePluginChangeKind::Skipped,
                        reason: "uninstall_failed".to_string(),
                    });
                    continue;
                }

                if let Err(error) = self
                    .controller
                    .install_plugin(&discovered_plugin.info, &discovered_plugin.capabilities)
                {
                    errors.push(format!(
                        "install plugin `{}` failed: {:#}",
                        plugin_id, error
                    ));
                    changes.push(RuntimePluginChange {
                        plugin_id: plugin_id.clone(),
                        kind: RuntimePluginChangeKind::Skipped,
                        reason: "install_failed".to_string(),
                    });
                    continue;
                }

                changes.push(RuntimePluginChange {
                    plugin_id: plugin_id.clone(),
                    kind: change_kind,
                    reason: if change_kind == RuntimePluginChangeKind::Reloaded {
                        "manifest_changed".to_string()
                    } else {
                        "new".to_string()
                    },
                });
            }

            next_active.insert(plugin_id.clone(), discovered_plugin.clone());
        }

        for plugin_id in previous_active.keys() {
            if next_active.contains_key(plugin_id) || discovered_active.contains_key(plugin_id) {
                continue;
            }
            if !try_uninstall_plugin(
                self.controller.as_ref(),
                plugin_id,
                PluginDisableReason::Unload,
                &mut deactivated,
                &mut errors,
            ) {
                changes.push(RuntimePluginChange {
                    plugin_id: plugin_id.clone(),
                    kind: RuntimePluginChangeKind::Skipped,
                    reason: "uninstall_failed".to_string(),
                });
                continue;
            }
            changes.push(RuntimePluginChange {
                plugin_id: plugin_id.clone(),
                kind: RuntimePluginChangeKind::Deactivated,
                reason: "removed".to_string(),
            });
        }

        let mut state = self
            .registry
            .write()
            .expect("runtime registry lock poisoned");
        state.revision = state.revision.saturating_add(1);
        let revision = state.revision;
        state.active_plugins = next_active;

        let mut active_plugins = state
            .active_plugins
            .values()
            .map(|plugin| plugin.info.clone())
            .collect::<Vec<_>>();
        active_plugins.sort_by(|a, b| a.id.cmp(&b.id));

        let discovered_ids = discovered_active.keys().cloned().collect::<BTreeSet<_>>();
        let plugin_statuses = build_plugin_statuses(
            &state.active_plugins,
            &discovered_ids,
            &state.desired_states,
        );

        changes.sort_by(|a, b| {
            a.plugin_id
                .cmp(&b.plugin_id)
                .then_with(|| a.reason.cmp(&b.reason))
        });

        for change in &changes {
            match change.kind {
                RuntimePluginChangeKind::Activated | RuntimePluginChangeKind::Reloaded => {
                    self.notify_plugin(&change.plugin_id, RuntimePluginDirective::Rebuild);
                },
                RuntimePluginChangeKind::Deactivated => {
                    let reason = match change.reason.as_str() {
                        "removed" => PluginDisableReason::Unload,
                        _ => PluginDisableReason::HostDisable,
                    };
                    self.notify_plugin(
                        &change.plugin_id,
                        RuntimePluginDirective::Destroy { reason },
                    );
                },
                RuntimePluginChangeKind::Skipped => {},
            }
        }

        Ok(RuntimeSyncReport {
            revision,
            discovered_plugins,
            active_plugins,
            plugin_statuses,
            changes,
            errors,
        })
    }
}

impl Drop for WasmPluginRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn try_uninstall_plugin(
    controller: &dyn WasmPluginController,
    plugin_id: &str,
    reason: PluginDisableReason,
    deactivated: &mut HashSet<String>,
    errors: &mut Vec<String>,
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
            errors.push(format!(
                "uninstall plugin `{}` failed: {:#}",
                plugin_id, error
            ));
            false
        },
    }
}
