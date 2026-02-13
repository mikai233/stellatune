use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use arc_swap::ArcSwap;
#[cfg(test)]
use stellatune_plugin_api::STELLATUNE_PLUGIN_API_VERSION;
use stellatune_plugin_api::StHostVTable;
use tokio::sync::mpsc;

use crate::events::{PluginEventBus, new_runtime_event_bus};
use crate::runtime::backend_control::BackendControlRequest;
use crate::runtime::introspection::{PluginLeaseInfo, PluginLeaseState};
use crate::runtime::model::{ModuleLease, ModuleLeaseRef};
use crate::runtime::registry::PluginModuleLeaseSlotState;

use super::load::RuntimePluginInfo;

mod host;
mod introspection;
mod shadow;
mod sync;

use self::introspection::RuntimeIntrospectionCache;

fn lease_id_of(lease: &Arc<ModuleLease>) -> u64 {
    Arc::as_ptr(lease) as usize as u64
}

pub struct PluginRuntimeService {
    host: StHostVTable,
    event_bus: PluginEventBus,
    modules: HashMap<String, PluginModuleLeaseSlotState>,
    disabled_plugin_ids: HashSet<String>,
    introspection_cache: ArcSwap<RuntimeIntrospectionCache>,
    introspection_cache_dirty: AtomicBool,
}

impl PluginRuntimeService {
    pub fn new(host: StHostVTable) -> Self {
        Self {
            host,
            event_bus: new_runtime_event_bus(),
            modules: HashMap::new(),
            disabled_plugin_ids: HashSet::new(),
            introspection_cache: ArcSwap::from_pointee(RuntimeIntrospectionCache::default()),
            introspection_cache_dirty: AtomicBool::new(false),
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

    pub fn subscribe_backend_control_requests(
        &self,
    ) -> mpsc::UnboundedReceiver<BackendControlRequest> {
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

    pub fn current_plugin_lease_info(&self, plugin_id: &str) -> Option<PluginLeaseInfo> {
        let slot = self.modules.get(plugin_id)?;
        let lease = slot.current.as_ref()?;
        Some(PluginLeaseInfo {
            lease_id: lease_id_of(lease),
            metadata_json: lease.metadata_json.clone(),
        })
    }

    pub fn plugin_lease_state(&self, plugin_id: &str) -> Option<PluginLeaseState> {
        let slot = self.modules.get(plugin_id)?;
        let current = slot.current.as_ref().map(|lease| PluginLeaseInfo {
            lease_id: lease_id_of(lease),
            metadata_json: lease.metadata_json.clone(),
        });
        let retired_lease_ids = slot.retired.iter().map(lease_id_of).collect::<Vec<_>>();
        Some(PluginLeaseState {
            current,
            retired_lease_ids,
        })
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
}

pub(crate) fn default_host_vtable() -> StHostVTable {
    host::default_host_vtable()
}

#[cfg(test)]
#[path = "tests/service_tests.rs"]
mod tests;
