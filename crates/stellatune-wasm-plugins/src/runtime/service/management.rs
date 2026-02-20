use std::collections::BTreeMap;

use crate::error::{Error, Result};

use crate::runtime::model::{DesiredPluginState, PluginDisableReason, RuntimePluginDirective};
use crate::runtime::service::WasmPluginRuntime;

impl WasmPluginRuntime {
    pub fn set_desired_state(
        &self,
        plugin_id: &str,
        desired_state: DesiredPluginState,
    ) -> Result<()> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Ok(());
        }

        if desired_state == DesiredPluginState::Disabled {
            let is_active = {
                let state = self
                    .registry
                    .read()
                    .expect("runtime registry lock poisoned");
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

        let mut state = self
            .registry
            .write()
            .expect("runtime registry lock poisoned");
        state
            .desired_states
            .insert(plugin_id.to_string(), desired_state);
        if desired_state == DesiredPluginState::Disabled {
            state.active_plugins.remove(plugin_id);
        }
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

        let active_ids = {
            let state = self
                .registry
                .read()
                .expect("runtime registry lock poisoned");
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

        let mut state = self
            .registry
            .write()
            .expect("runtime registry lock poisoned");
        state.desired_states = normalized;
        for plugin_id in deactivated_ids {
            state.active_plugins.remove(&plugin_id);
        }

        if deactivation_errors.is_empty() {
            return Ok(());
        }
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
        let is_active = {
            let state = self
                .registry
                .read()
                .expect("runtime registry lock poisoned");
            state.active_plugins.contains_key(plugin_id)
        };
        if !is_active {
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

        let mut state = self
            .registry
            .write()
            .expect("runtime registry lock poisoned");
        Ok(state.active_plugins.remove(plugin_id).is_some())
    }

    pub fn shutdown(&self) -> Result<Vec<String>> {
        let active_ids = {
            let state = self
                .registry
                .read()
                .expect("runtime registry lock poisoned");
            let mut ids = state.active_plugins.keys().cloned().collect::<Vec<_>>();
            ids.sort();
            ids
        };

        let mut deactivated = Vec::<String>::new();
        let mut errors = Vec::<String>::new();
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

        let mut state = self
            .registry
            .write()
            .expect("runtime registry lock poisoned");
        for plugin_id in &deactivated {
            state.active_plugins.remove(plugin_id);
        }

        if errors.is_empty() {
            Ok(deactivated)
        } else {
            Err(Error::aggregate("shutdown", errors))
        }
    }
}
