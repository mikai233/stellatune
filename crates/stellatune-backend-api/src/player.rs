use anyhow::{Result, anyhow};

use crate::runtime::shared_plugin_runtime;

pub fn plugins_install_from_file(plugins_dir: String, artifact_path: String) -> Result<String> {
    let installed = stellatune_plugins::install_plugin_from_artifact(&plugins_dir, &artifact_path)
        .map_err(|e| anyhow!(e.to_string()))?;
    Ok(installed.id)
}

pub fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    let installed = stellatune_plugins::list_installed_plugins(&plugins_dir)
        .map_err(|e| anyhow!(e.to_string()))?;
    serde_json::to_string(&installed).map_err(|e| anyhow!(e.to_string()))
}

pub fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }

    ensure_plugin_runtime_unloaded_for_uninstall(&plugin_id)?;

    stellatune_plugins::uninstall_plugin(&plugins_dir, &plugin_id)
        .map_err(|e| anyhow!(e.to_string()))
}

fn ensure_plugin_runtime_unloaded_for_uninstall(plugin_id: &str) -> Result<()> {
    let service = shared_plugin_runtime();

    let _ = stellatune_runtime::block_on(service.unload_plugin(plugin_id));
    let Some(state) = stellatune_runtime::block_on(service.plugin_lease_state(plugin_id)) else {
        return Ok(());
    };
    if !state.retired_lease_ids.is_empty() {
        return Err(anyhow!(
            "plugin `{plugin_id}` is still in use ({} retired lease(s)); stop playback/release instances and retry uninstall",
            state.retired_lease_ids.len()
        ));
    }
    Ok(())
}
