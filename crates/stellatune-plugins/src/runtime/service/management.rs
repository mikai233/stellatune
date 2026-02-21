use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::executor::WasmPluginController;

use crate::runtime::model::{DesiredPluginState, PluginDisableReason, RuntimePluginDirective};
use crate::runtime::service::WasmPluginRuntime;

impl<C: WasmPluginController> WasmPluginRuntime<C> {
    pub fn set_desired_state(
        &self,
        plugin_id: &str,
        desired_state: DesiredPluginState,
    ) -> Result<()> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Ok(());
        }
        tracing::info!(
            target: "stellatune_plugins::runtime",
            plugin_id = %plugin_id,
            desired_state = ?desired_state,
            "plugin desired state update requested"
        );

        if desired_state == DesiredPluginState::Disabled {
            let is_active = {
                let state = self.registry.read();
                state.active_plugins.contains_key(plugin_id)
            };
            if is_active {
                self.controller
                    .uninstall_plugin(plugin_id, PluginDisableReason::HostDisable)?;
                self.notify_plugin(
                    plugin_id,
                    RuntimePluginDirective::Destroy {
                        reason: PluginDisableReason::HostDisable,
                    },
                );
            }
        }

        let mut state = self.registry.write();
        state
            .desired_states
            .insert(plugin_id.to_string(), desired_state);
        if desired_state == DesiredPluginState::Disabled {
            state.active_plugins.remove(plugin_id);
        }
        tracing::info!(
            target: "stellatune_plugins::runtime",
            plugin_id = %plugin_id,
            desired_state = ?desired_state,
            "plugin desired state updated"
        );
        Ok(())
    }

    pub fn replace_desired_states(
        &self,
        desired_states: BTreeMap<String, DesiredPluginState>,
    ) -> Result<()> {
        let mut normalized = BTreeMap::<String, DesiredPluginState>::new();
        for (plugin_id, state_value) in desired_states {
            let plugin_id = plugin_id.trim();
            if plugin_id.is_empty() {
                continue;
            }
            normalized.insert(plugin_id.to_string(), state_value);
        }
        tracing::info!(
            target: "stellatune_plugins::runtime",
            desired_entries = normalized.len(),
            "plugin desired state replacement requested"
        );

        let active_ids = {
            let state = self.registry.read();
            state.active_plugins.keys().cloned().collect::<Vec<_>>()
        };

        let mut deactivation_errors = Vec::<String>::new();
        let mut deactivated_ids = Vec::<String>::new();
        for plugin_id in active_ids {
            let desired_state = normalized
                .get(&plugin_id)
                .copied()
                .unwrap_or(DesiredPluginState::Enabled);
            if desired_state != DesiredPluginState::Disabled {
                continue;
            }
            match self
                .controller
                .uninstall_plugin(&plugin_id, PluginDisableReason::HostDisable)
            {
                Ok(()) => {
                    self.notify_plugin(
                        &plugin_id,
                        RuntimePluginDirective::Destroy {
                            reason: PluginDisableReason::HostDisable,
                        },
                    );
                    deactivated_ids.push(plugin_id);
                },
                Err(error) => {
                    deactivation_errors.push(format!(
                        "uninstall plugin `{}` failed: {:#}",
                        plugin_id, error
                    ));
                },
            }
        }

        let mut state = self.registry.write();
        state.desired_states = normalized;
        for plugin_id in deactivated_ids {
            state.active_plugins.remove(&plugin_id);
        }

        if deactivation_errors.is_empty() {
            tracing::info!(
                target: "stellatune_plugins::runtime",
                "plugin desired state replacement completed"
            );
            return Ok(());
        }
        tracing::warn!(
            target: "stellatune_plugins::runtime",
            error_count = deactivation_errors.len(),
            "plugin desired state replacement completed with errors"
        );
        Err(Error::aggregate(
            "replace_desired_states",
            deactivation_errors,
        ))
    }

    pub fn uninstall_plugin(&self, plugin_id: &str) -> Result<bool> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Ok(false);
        }
        tracing::info!(
            target: "stellatune_plugins::runtime",
            plugin_id = %plugin_id,
            "plugin uninstall requested"
        );
        let is_active = {
            let state = self.registry.read();
            state.active_plugins.contains_key(plugin_id)
        };
        if !is_active {
            tracing::debug!(
                target: "stellatune_plugins::runtime",
                plugin_id = %plugin_id,
                "plugin uninstall skipped because plugin is not active"
            );
            return Ok(false);
        }
        self.controller
            .uninstall_plugin(plugin_id, PluginDisableReason::HostDisable)?;
        self.notify_plugin(
            plugin_id,
            RuntimePluginDirective::Destroy {
                reason: PluginDisableReason::HostDisable,
            },
        );

        let mut state = self.registry.write();
        let removed = state.active_plugins.remove(plugin_id).is_some();
        tracing::info!(
            target: "stellatune_plugins::runtime",
            plugin_id = %plugin_id,
            removed,
            "plugin uninstall completed"
        );
        Ok(removed)
    }

    pub fn shutdown(&self) -> Result<Vec<String>> {
        let active_ids = {
            let state = self.registry.read();
            let mut ids = state.active_plugins.keys().cloned().collect::<Vec<_>>();
            ids.sort();
            ids
        };

        let mut deactivated = Vec::<String>::new();
        let mut errors = Vec::<String>::new();
        tracing::info!(
            target: "stellatune_plugins::runtime",
            active_before = active_ids.len(),
            "plugin runtime shutdown requested"
        );
        for plugin_id in &active_ids {
            match self
                .controller
                .uninstall_plugin(plugin_id, PluginDisableReason::Shutdown)
            {
                Ok(()) => {
                    self.notify_plugin(
                        plugin_id,
                        RuntimePluginDirective::Destroy {
                            reason: PluginDisableReason::Shutdown,
                        },
                    );
                    deactivated.push(plugin_id.clone())
                },
                Err(error) => errors.push(format!(
                    "uninstall plugin `{}` failed during shutdown: {:#}",
                    plugin_id, error
                )),
            }
        }
        if let Err(error) = self.controller.shutdown() {
            errors.push(format!("runtime host shutdown failed: {:#}", error));
        }

        let mut state = self.registry.write();
        for plugin_id in &deactivated {
            state.active_plugins.remove(plugin_id);
        }

        if errors.is_empty() {
            tracing::info!(
                target: "stellatune_plugins::runtime",
                deactivated = deactivated.len(),
                "plugin runtime shutdown completed"
            );
            Ok(deactivated)
        } else {
            tracing::warn!(
                target: "stellatune_plugins::runtime",
                deactivated = deactivated.len(),
                errors = errors.len(),
                "plugin runtime shutdown completed with errors"
            );
            Err(Error::aggregate("shutdown", errors))
        }
    }
}
