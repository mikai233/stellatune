use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::runtime::shared_plugin_manager;

#[derive(Serialize)]
struct InstalledPluginView {
    id: String,
    name: String,
    version: String,
    install_state: &'static str,
    manifest_version: u32,
}

pub async fn plugins_install_from_file(
    plugins_dir: String,
    artifact_path: String,
) -> Result<String> {
    let manager = shared_plugin_manager(Path::new(&plugins_dir));
    let installed = manager
        .install(PathBuf::from(artifact_path))
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(installed.id)
}

pub fn plugins_list_installed_json(plugins_dir: String) -> Result<String> {
    let installed = stellatune_plugins::typescript::package::discover_typescript_plugins(
        Path::new(&plugins_dir),
    )
    .map_err(|error| anyhow!(error.to_string()))?;
    let installed = installed
        .into_iter()
        .map(|plugin| InstalledPluginView {
            id: plugin.manifest.id,
            name: plugin.manifest.name,
            version: plugin.manifest.version,
            install_state: "installed",
            manifest_version: 2,
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&installed).map_err(|error| anyhow!(error.to_string()))
}

pub async fn plugins_uninstall_by_id(plugins_dir: String, plugin_id: String) -> Result<()> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }
    let manager = shared_plugin_manager(Path::new(&plugins_dir));
    manager
        .uninstall(plugin_id)
        .await
        .map_err(|error| anyhow!(error.to_string()))
}
