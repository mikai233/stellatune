use std::sync::Arc;

use anyhow::{Result, anyhow};
use crossbeam_channel::Receiver;

use crate::capabilities::common::{InstanceRuntimeCtx, PluginFreeFn};
use crate::runtime::handle::{ModuleLeaseHandle, PluginRuntimeHandle};
use crate::runtime::instance_registry::InstanceRegistry;
use crate::runtime::messages::WorkerControlMessage;
use crate::runtime::update::InstanceUpdateCoordinator;

pub(super) fn normalize_plugin_type_ids(
    plugin_id: &str,
    type_id: &str,
) -> Result<(String, String)> {
    let plugin_id = plugin_id.trim().to_string();
    let type_id = type_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }
    if type_id.is_empty() {
        return Err(anyhow!("type_id is empty"));
    }
    Ok((plugin_id, type_id))
}

pub(super) fn subscribe_worker_control(
    runtime: &PluginRuntimeHandle,
    plugin_id: &str,
) -> Result<Receiver<WorkerControlMessage>> {
    let (control_tx, control_rx) = crossbeam_channel::unbounded();
    if !stellatune_runtime::block_on(runtime.register_worker_control_sender(plugin_id, control_tx))
    {
        return Err(anyhow!("plugin runtime actor unavailable"));
    }
    Ok(control_rx)
}

pub(super) fn acquire_active_lease(
    runtime: &PluginRuntimeHandle,
    plugin_id: &str,
) -> Result<ModuleLeaseHandle> {
    stellatune_runtime::block_on(runtime.acquire_current_module_lease(plugin_id)).ok_or_else(|| {
        anyhow!(
            "plugin `{}` has no active lease (disabled, unloaded, or unavailable)",
            plugin_id
        )
    })
}

pub(super) fn new_factory_state() -> (Arc<InstanceRegistry>, Arc<InstanceUpdateCoordinator>) {
    (
        Arc::new(InstanceRegistry::default()),
        Arc::new(InstanceUpdateCoordinator::default()),
    )
}

pub(super) fn new_instance_runtime_ctx(
    instances: &Arc<InstanceRegistry>,
    updates: &Arc<InstanceUpdateCoordinator>,
    lease: ModuleLeaseHandle,
    plugin_free: PluginFreeFn,
) -> InstanceRuntimeCtx {
    let instance_id = instances.register();
    InstanceRuntimeCtx {
        instance_id,
        module_lease: lease,
        updates: Arc::clone(updates),
        plugin_free,
    }
}
