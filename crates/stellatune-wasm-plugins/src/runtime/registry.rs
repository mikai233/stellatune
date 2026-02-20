use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::manifest::{ComponentSpec, WasmPluginManifest};
use crate::runtime::model::{
    DesiredPluginState, RuntimeCapabilityDescriptor, RuntimePluginInfo, RuntimePluginState,
    RuntimePluginStatus,
};

#[derive(Debug, Clone)]
pub(crate) struct ActivePlugin {
    pub(crate) info: RuntimePluginInfo,
    pub(crate) capabilities: Vec<RuntimeCapabilityDescriptor>,
    pub(crate) signature: String,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeRegistry {
    pub(crate) revision: u64,
    pub(crate) active_plugins: BTreeMap<String, ActivePlugin>,
    pub(crate) desired_states: BTreeMap<String, DesiredPluginState>,
}

pub(crate) fn build_plugin_statuses(
    active_plugins: &BTreeMap<String, ActivePlugin>,
    discovered_plugin_ids: &BTreeSet<String>,
    desired_states: &BTreeMap<String, DesiredPluginState>,
) -> Vec<RuntimePluginStatus> {
    let mut plugin_ids = BTreeSet::<String>::new();
    plugin_ids.extend(active_plugins.keys().cloned());
    plugin_ids.extend(discovered_plugin_ids.iter().cloned());
    plugin_ids.extend(desired_states.keys().cloned());

    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let desired_state = desired_states
            .get(&plugin_id)
            .copied()
            .unwrap_or(DesiredPluginState::Enabled);
        let runtime_state = if active_plugins.contains_key(&plugin_id) {
            RuntimePluginState::Active
        } else if discovered_plugin_ids.contains(&plugin_id) {
            RuntimePluginState::Inactive
        } else {
            RuntimePluginState::Missing
        };
        out.push(RuntimePluginStatus {
            plugin_id,
            desired_state,
            runtime_state,
        });
    }
    out
}

pub(crate) fn active_plugin_from_manifest(
    root_dir: PathBuf,
    manifest_path: PathBuf,
    manifest: WasmPluginManifest,
) -> ActivePlugin {
    let info = RuntimePluginInfo {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        root_dir,
        manifest_path,
        component_count: manifest.components.len(),
    };
    let capabilities = manifest
        .components
        .iter()
        .flat_map(|component| capabilities_from_component(&manifest.id, component))
        .collect::<Vec<_>>();
    let signature = plugin_signature(&manifest);
    ActivePlugin {
        info,
        capabilities,
        signature,
    }
}

fn capabilities_from_component(
    plugin_id: &str,
    component: &ComponentSpec,
) -> Vec<RuntimeCapabilityDescriptor> {
    component
        .abilities
        .iter()
        .map(|ability| RuntimeCapabilityDescriptor {
            plugin_id: plugin_id.to_string(),
            component_id: component.id.clone(),
            component_rel_path: component.path.clone(),
            world: component.world.clone(),
            kind: ability.kind,
            type_id: ability.type_id.clone(),
            threading: component.threading.clone(),
        })
        .collect()
}

fn plugin_signature(manifest: &WasmPluginManifest) -> String {
    let mut out = String::new();
    out.push_str(manifest.id.as_str());
    out.push('|');
    out.push_str(manifest.version.as_str());
    out.push('|');
    out.push_str(manifest.name.as_str());
    out.push('|');
    out.push_str(manifest.api_version.to_string().as_str());
    for component in &manifest.components {
        out.push('|');
        out.push_str(component.id.as_str());
        out.push('|');
        out.push_str(component.path.as_str());
        out.push('|');
        out.push_str(component.world.as_str());
        for ability in &component.abilities {
            out.push('|');
            out.push_str(format!("{:?}", ability.kind).as_str());
            out.push(':');
            out.push_str(ability.type_id.as_str());
        }
    }
    out
}
