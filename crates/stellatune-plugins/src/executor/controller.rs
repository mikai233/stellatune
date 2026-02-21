use std::collections::BTreeMap;

use crate::error::Result;

use crate::executor::{
    ActivePluginRecord, WasmPluginController, WasmtimePluginController, WorldKind, classify_world,
};
use crate::runtime::model::{
    PluginDisableReason, RuntimeCapabilityDescriptor, RuntimePluginDirective, RuntimePluginInfo,
};

const PACKAGE_SIDECAR_SHUTDOWN_GRACE_MS: u32 = 1_500;

impl WasmPluginController for WasmtimePluginController {
    fn install_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
    ) -> Result<()> {
        let mut force_rebuild = false;
        let mut dedup = BTreeMap::<String, (String, String)>::new();
        for capability in capabilities {
            match dedup.entry(capability.component_id.clone()) {
                std::collections::btree_map::Entry::Vacant(vacant) => {
                    vacant.insert((
                        capability.component_rel_path.clone(),
                        capability.world.clone(),
                    ));
                },
                std::collections::btree_map::Entry::Occupied(mut existing) => {
                    let (known_path, known_world) = existing.get();
                    if known_path != &capability.component_rel_path
                        || known_world != &capability.world
                    {
                        force_rebuild = true;
                        existing.insert((
                            capability.component_rel_path.clone(),
                            capability.world.clone(),
                        ));
                    }
                },
            }
        }

        for (component_id, (component_rel_path, world)) in dedup {
            if classify_world(&world) == WorldKind::Unknown {
                return Err(crate::op_error!(
                    "unsupported world `{}` in plugin `{}` component `{}`",
                    world,
                    plugin.id,
                    component_id
                ));
            }
            let component_path: std::path::PathBuf = plugin.root_dir.join(&component_rel_path);
            self.load_component_cached(&component_path).map_err(|e| {
                crate::op_error!(
                    "failed to load component for plugin `{}` component `{}`: {e:#}",
                    plugin.id,
                    component_id
                )
            })?;
        }

        let mut routes = self.directives.write();
        let already_installed = routes.active_plugins.contains(&plugin.id);
        routes.active_plugins.insert(plugin.id.clone());
        let senders = routes.senders.entry(plugin.id.clone()).or_default();
        if already_installed || force_rebuild {
            senders.retain(|sender| sender.send(RuntimePluginDirective::Rebuild).is_ok());
        }
        drop(routes);
        if !already_installed {
            self.sidecar_registry.plugin_activated(plugin.id.as_str());
        }

        let mut plugins = self.plugins.write();
        plugins.insert(
            plugin.id.clone(),
            ActivePluginRecord {
                info: plugin.clone(),
                capabilities: capabilities.to_vec(),
            },
        );
        Ok(())
    }

    fn uninstall_plugin(&self, plugin_id: &str, reason: PluginDisableReason) -> Result<()> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Ok(());
        }
        let mut routes = self.directives.write();
        if let Some(senders) = routes.senders.get_mut(plugin_id) {
            senders.retain(|sender| {
                sender
                    .send(RuntimePluginDirective::Destroy { reason })
                    .is_ok()
            });
        }
        let was_active = routes.active_plugins.remove(plugin_id);
        routes.senders.remove(plugin_id);
        drop(routes);
        if was_active {
            // Package-scoped sidecars must follow plugin enabled/disabled lifecycle.
            self.sidecar_registry
                .plugin_deactivated(plugin_id, PACKAGE_SIDECAR_SHUTDOWN_GRACE_MS);
        }

        let mut plugins = self.plugins.write();
        let removed = plugins.remove(plugin_id);
        drop(plugins);
        if let Some(record) = removed {
            self.remove_cached_components_for_plugin(&record.info, &record.capabilities);
        }
        Ok(())
    }

    fn dispatch_directive(&self, plugin_id: &str, directive: RuntimePluginDirective) -> Result<()> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Ok(());
        }
        let senders_to_dispatch = {
            let routes = self.directives.read();
            if !routes.active_plugins.contains(plugin_id) {
                return Ok(());
            }
            let Some(senders) = routes.senders.get(plugin_id) else {
                return Ok(());
            };
            senders.clone()
        };

        for sender in senders_to_dispatch {
            let _ = sender.send(directive.clone());
        }
        Ok(())
    }

    fn shutdown(&self) -> Result<()> {
        let mut routes = self.directives.write();
        let active_plugin_ids = routes.active_plugins.iter().cloned().collect::<Vec<_>>();
        for senders in routes.senders.values_mut() {
            senders.retain(|sender| {
                sender
                    .send(RuntimePluginDirective::Destroy {
                        reason: PluginDisableReason::Shutdown,
                    })
                    .is_ok()
            });
        }
        routes.active_plugins.clear();
        routes.senders.clear();
        drop(routes);
        for plugin_id in active_plugin_ids {
            self.sidecar_registry
                .plugin_deactivated(plugin_id.as_str(), PACKAGE_SIDECAR_SHUTDOWN_GRACE_MS);
        }

        let mut plugins = self.plugins.write();
        plugins.clear();
        drop(plugins);
        self.clear_component_cache();
        Ok(())
    }
}

impl Drop for WasmtimePluginController {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
