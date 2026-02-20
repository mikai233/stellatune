use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{ErrorContext, Result};
use serde::{Deserialize, Serialize};
use tracing::warn;

pub const PLUGIN_MANIFEST_FILE_NAME: &str = "plugin.json";
pub const INSTALL_RECEIPT_FILE_NAME: &str = ".install.json";
pub const UNINSTALL_PENDING_MARKER_FILE_NAME: &str = ".uninstall-pending";
const DELETE_FAILED_RETRY_THRESHOLD: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInstallState {
    Installed,
    PendingUninstall,
    DeleteFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbilityKind {
    Decoder,
    Source,
    Lyrics,
    OutputSink,
    Dsp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilitySpec {
    pub kind: AbilityKind,
    pub type_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadingModel {
    Dedicated,
    SharedPool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadingHint {
    pub model: ThreadingModel,
    #[serde(default)]
    pub max_instances: Option<u32>,
    #[serde(default)]
    pub pool: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub id: String,
    pub path: String,
    pub world: String,
    pub abilities: Vec<AbilitySpec>,
    #[serde(default)]
    pub threading: Option<ThreadingHint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmPluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: u32,
    pub components: Vec<ComponentSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInstallReceipt {
    pub manifest: WasmPluginManifest,
    pub manifest_rel_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallPendingMarker {
    pub plugin_id: String,
    pub queued_at_ms: u64,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default = "default_pending_uninstall_state")]
    pub state: PluginInstallState,
}

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub root_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: WasmPluginManifest,
}

#[derive(Debug, Clone)]
pub struct PendingUninstallPlugin {
    pub root_dir: PathBuf,
    pub marker: UninstallPendingMarker,
    pub receipt: Option<PluginInstallReceipt>,
}

fn default_pending_uninstall_state() -> PluginInstallState {
    PluginInstallState::PendingUninstall
}

pub fn receipt_path_for_plugin_root(root: &Path) -> PathBuf {
    root.join(INSTALL_RECEIPT_FILE_NAME)
}

pub fn pending_marker_path_for_plugin_root(root: &Path) -> PathBuf {
    root.join(UNINSTALL_PENDING_MARKER_FILE_NAME)
}

pub fn read_manifest(path: &Path) -> Result<WasmPluginManifest> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let manifest = serde_json::from_str::<WasmPluginManifest>(&raw)
        .with_context(|| format!("parse {}", path.display()))?;
    validate_manifest(&manifest, path.parent().unwrap_or_else(|| Path::new(".")))?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &WasmPluginManifest, root_dir: &Path) -> Result<()> {
    if manifest.schema_version != 1 {
        return Err(crate::op_error!(
            "unsupported plugin manifest schema_version: {}",
            manifest.schema_version
        ));
    }
    if manifest.id.trim().is_empty() {
        return Err(crate::op_error!("manifest.id is empty"));
    }
    if manifest.name.trim().is_empty() {
        return Err(crate::op_error!("manifest.name is empty"));
    }
    if manifest.version.trim().is_empty() {
        return Err(crate::op_error!("manifest.version is empty"));
    }
    if manifest.components.is_empty() {
        return Err(crate::op_error!("manifest.components is empty"));
    }

    let mut component_ids = HashSet::<String>::new();
    let mut ability_keys = HashSet::<(AbilityKind, String)>::new();

    for component in &manifest.components {
        let component_id = component.id.trim();
        if component_id.is_empty() {
            return Err(crate::op_error!("component.id is empty"));
        }
        if !component_ids.insert(component_id.to_string()) {
            return Err(crate::op_error!("duplicate component.id: {component_id}"));
        }
        if component.path.trim().is_empty() {
            return Err(crate::op_error!("component `{component_id}` path is empty"));
        }
        let rel = Path::new(component.path.trim());
        if rel.is_absolute()
            || rel
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(crate::op_error!(
                "component `{component_id}` path is unsafe: {}",
                component.path
            ));
        }
        let resolved = root_dir.join(rel);
        if !resolved.exists() {
            return Err(crate::op_error!(
                "component `{component_id}` path not found: {}",
                resolved.display()
            ));
        }
        if component.world.trim().is_empty() {
            return Err(crate::op_error!(
                "component `{component_id}` world is empty"
            ));
        }
        if component.abilities.is_empty() {
            return Err(crate::op_error!(
                "component `{component_id}` abilities is empty"
            ));
        }
        for ability in &component.abilities {
            let type_id = ability.type_id.trim();
            if type_id.is_empty() {
                return Err(crate::op_error!(
                    "component `{component_id}` has empty ability type_id"
                ));
            }
            let key = (ability.kind, type_id.to_string());
            if !ability_keys.insert(key) {
                return Err(crate::op_error!(
                    "duplicate ability key detected in plugin `{}`: {:?}/{}",
                    manifest.id,
                    ability.kind,
                    type_id
                ));
            }
        }
    }

    Ok(())
}

pub fn write_receipt(root: &Path, receipt: &PluginInstallReceipt) -> Result<()> {
    let path = receipt_path_for_plugin_root(root);
    let text = serde_json::to_string_pretty(receipt).context("serialize install receipt")?;
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

pub fn read_receipt(path: &Path) -> Result<PluginInstallReceipt> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

pub fn write_uninstall_pending_marker(path: &Path, marker: &UninstallPendingMarker) -> Result<()> {
    let text =
        serde_json::to_string_pretty(marker).context("serialize uninstall pending marker")?;
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))
}

pub fn read_uninstall_pending_marker(path: &Path) -> Result<UninstallPendingMarker> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

pub fn discover_plugins(dir: impl AsRef<Path>) -> Result<Vec<DiscoveredPlugin>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    try_cleanup_pending_uninstalls(dir);

    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy() != INSTALL_RECEIPT_FILE_NAME {
            continue;
        }

        let receipt_path = entry.path().to_path_buf();
        let Some(root_dir) = receipt_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        let marker = pending_marker_path_for_plugin_root(&root_dir);
        if marker.exists() {
            continue;
        }

        let receipt = match read_receipt(&receipt_path) {
            Ok(v) => v,
            Err(err) => {
                warn!(
                    target: "stellatune_wasm_plugins::discover",
                    receipt = %receipt_path.display(),
                    "skip unreadable install receipt: {err:#}"
                );
                continue;
            },
        };
        let manifest_path = root_dir.join(&receipt.manifest_rel_path);
        if !manifest_path.exists() {
            warn!(
                target: "stellatune_wasm_plugins::discover",
                plugin_id = %receipt.manifest.id,
                manifest_path = %manifest_path.display(),
                "skip plugin with missing manifest path"
            );
            continue;
        }
        let manifest = match read_manifest(&manifest_path) {
            Ok(v) => v,
            Err(err) => {
                warn!(
                    target: "stellatune_wasm_plugins::discover",
                    plugin_id = %receipt.manifest.id,
                    manifest_path = %manifest_path.display(),
                    "skip plugin with invalid manifest: {err:#}"
                );
                continue;
            },
        };
        out.push(DiscoveredPlugin {
            root_dir,
            manifest_path,
            manifest,
        });
    }
    Ok(out)
}

pub fn discover_pending_uninstalls(dir: impl AsRef<Path>) -> Result<Vec<PendingUninstallPlugin>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy() != UNINSTALL_PENDING_MARKER_FILE_NAME {
            continue;
        }
        let marker_path = entry.path().to_path_buf();
        let Some(root_dir) = marker_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        let marker = read_uninstall_pending_marker(&marker_path)
            .unwrap_or_else(|_| default_marker_for_root(&root_dir));
        let receipt_path = receipt_path_for_plugin_root(&root_dir);
        let receipt = read_receipt(&receipt_path).ok();
        out.push(PendingUninstallPlugin {
            root_dir,
            marker,
            receipt,
        });
    }
    out.sort_by(|a, b| a.marker.plugin_id.cmp(&b.marker.plugin_id));
    Ok(out)
}

fn try_cleanup_pending_uninstalls(dir: &Path) {
    let mut marker_paths = Vec::<PathBuf>::new();
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy() != UNINSTALL_PENDING_MARKER_FILE_NAME {
            continue;
        }
        marker_paths.push(entry.path().to_path_buf());
    }

    marker_paths.sort();
    marker_paths.dedup();

    for marker_path in marker_paths {
        let Some(root_dir) = marker_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        let mut marker = read_uninstall_pending_marker(&marker_path)
            .unwrap_or_else(|_| default_marker_for_root(&root_dir));
        if let Err(err) = std::fs::remove_dir_all(&root_dir) {
            marker.retry_count = marker.retry_count.saturating_add(1);
            marker.last_error = Some(err.to_string());
            marker.state = if marker.retry_count >= DELETE_FAILED_RETRY_THRESHOLD {
                PluginInstallState::DeleteFailed
            } else {
                PluginInstallState::PendingUninstall
            };
            let _ = write_uninstall_pending_marker(&marker_path, &marker);
            warn!(
                target: "stellatune_wasm_plugins::discover",
                root = %root_dir.display(),
                "cleanup pending uninstall failed: {err:#}"
            );
        }
    }
}

fn default_marker_for_root(root: &Path) -> UninstallPendingMarker {
    UninstallPendingMarker {
        plugin_id: plugin_id_from_root(root),
        queued_at_ms: 0,
        retry_count: 0,
        last_error: None,
        state: PluginInstallState::PendingUninstall,
    }
}

fn plugin_id_from_root(root: &Path) -> String {
    root.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown-plugin".to_string())
}
