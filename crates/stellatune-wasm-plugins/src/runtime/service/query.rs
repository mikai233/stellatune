use std::collections::BTreeSet;

use crate::manifest::AbilityKind;
use crate::runtime::model::{
    DesiredPluginState, RuntimeCapabilityDescriptor, RuntimePluginInfo, RuntimePluginStatus,
};
use crate::runtime::registry::build_plugin_statuses;
use crate::runtime::service::WasmPluginRuntime;

impl WasmPluginRuntime {
    pub fn desired_state(&self, plugin_id: &str) -> DesiredPluginState {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return DesiredPluginState::Enabled;
        }
        let state = self
            .registry
            .read()
            .expect("runtime registry lock poisoned");
        state
            .desired_states
            .get(plugin_id)
            .copied()
            .unwrap_or(DesiredPluginState::Enabled)
    }

    pub fn plugin_statuses(&self) -> Vec<RuntimePluginStatus> {
        let state = self
            .registry
            .read()
            .expect("runtime registry lock poisoned");
        let discovered_ids = state
            .active_plugins
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        build_plugin_statuses(
            &state.active_plugins,
            &discovered_ids,
            &state.desired_states,
        )
    }

    pub fn active_plugins(&self) -> Vec<RuntimePluginInfo> {
        let state = self
            .registry
            .read()
            .expect("runtime registry lock poisoned");
        let mut out = state
            .active_plugins
            .values()
            .map(|plugin| plugin.info.clone())
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn active_ids(&self) -> Vec<String> {
        let state = self
            .registry
            .read()
            .expect("runtime registry lock poisoned");
        let mut out = state.active_plugins.keys().cloned().collect::<Vec<_>>();
        out.sort();
        out
    }

    pub fn capabilities_of(&self, plugin_id: &str) -> Vec<RuntimeCapabilityDescriptor> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Vec::new();
        }
        let state = self
            .registry
            .read()
            .expect("runtime registry lock poisoned");
        let Some(plugin) = state.active_plugins.get(plugin_id) else {
            return Vec::new();
        };
        plugin.capabilities.clone()
    }

    pub fn capability_of(
        &self,
        plugin_id: &str,
        kind: AbilityKind,
        type_id: &str,
    ) -> Option<RuntimeCapabilityDescriptor> {
        let plugin_id = plugin_id.trim();
        let type_id = type_id.trim();
        if plugin_id.is_empty() || type_id.is_empty() {
            return None;
        }
        let state = self
            .registry
            .read()
            .expect("runtime registry lock poisoned");
        let plugin = state.active_plugins.get(plugin_id)?;
        plugin
            .capabilities
            .iter()
            .find(|cap| cap.kind == kind && cap.type_id == type_id)
            .cloned()
    }
}
