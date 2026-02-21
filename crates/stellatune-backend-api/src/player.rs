use anyhow::{Result, anyhow};

use crate::runtime::shared_plugin_runtime;

pub fn plugins_install_from_file(plugins_dir: String, artifact_path: String) -> Result<String> {
    let installed =
        stellatune_plugins::package::install_from_artifact(&plugins_dir, &artifact_path)
            .map_err(|e| anyhow!(e.to_string()))?;
    sync_runtime_after_package_change(&plugins_dir)?;
    Ok(installed.id)
}

pub fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    let installed = stellatune_plugins::package::list_installed(&plugins_dir)
        .map_err(|e| anyhow!(e.to_string()))?;
    serde_json::to_string(&installed).map_err(|e| anyhow!(e.to_string()))
}

pub fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    let plugin_id = plugin_id.trim().to_string();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }

    ensure_plugin_runtime_unloaded_for_uninstall(&plugin_id)?;

    stellatune_plugins::package::uninstall_by_id(&plugins_dir, &plugin_id)
        .map_err(|e| anyhow!(e.to_string()))?;
    sync_runtime_after_package_change(&plugins_dir)
}

fn ensure_plugin_runtime_unloaded_for_uninstall(plugin_id: &str) -> Result<()> {
    let service = shared_plugin_runtime();

    let _ = stellatune_runtime::block_on(service.unload_plugin_report(plugin_id));
    let mut active_plugin_ids = service.active_plugin_ids();
    active_plugin_ids.sort();
    if active_plugin_ids.iter().any(|id| id == plugin_id) {
        return Err(anyhow!(
            "plugin `{plugin_id}` is still active after unload; stop playback/release instances and retry uninstall"
        ));
    }
    Ok(())
}

fn sync_runtime_after_package_change(plugins_dir: &str) -> Result<()> {
    let service = shared_plugin_runtime();
    let report = stellatune_runtime::block_on(service.reload_dir_detailed_from_state(plugins_dir))
        .map_err(|error| {
            anyhow!("failed to sync wasm plugin runtime after package change: {error:#}")
        })?;
    if report.load_report.errors.is_empty() {
        return Ok(());
    }

    let details = report
        .load_report
        .errors
        .into_iter()
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>()
        .join("; ");
    Err(anyhow!(
        "wasm plugin runtime sync completed with errors after package change: {details}"
    ))
}
