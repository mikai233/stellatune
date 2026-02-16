use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use stellatune_plugin_api::PluginMetadata;
use tracing::warn;

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
pub struct PendingUninstallPlugin {
    pub root_dir: PathBuf,
    pub marker: UninstallPendingMarker,
    pub receipt: Option<PluginInstallReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub api_version: u32,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub entry_symbol: Option<String>,

    #[serde(default)]
    pub metadata: Option<PluginMetadata>,
}

impl PluginManifest {
    pub fn entry_symbol(&self) -> &str {
        self.entry_symbol
            .as_deref()
            .unwrap_or(stellatune_plugin_api::STELLATUNE_PLUGIN_ENTRY_SYMBOL)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInstallReceipt {
    pub manifest: PluginManifest,
    pub library_rel_path: String,
}

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub root_dir: PathBuf,
    pub manifest: PluginManifest,
    pub library_path: PathBuf,
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

pub fn write_receipt(root: &Path, receipt: &PluginInstallReceipt) -> Result<()> {
    let path = receipt_path_for_plugin_root(root);
    let text = serde_json::to_string_pretty(receipt).context("serialize plugin install receipt")?;
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))
}

pub fn read_receipt(path: &Path) -> Result<PluginInstallReceipt> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str::<PluginInstallReceipt>(&text)
        .with_context(|| format!("parse {}", path.display()))
}

pub fn write_uninstall_pending_marker(path: &Path, marker: &UninstallPendingMarker) -> Result<()> {
    let text =
        serde_json::to_string_pretty(marker).context("serialize uninstall pending marker")?;
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))
}

pub fn read_uninstall_pending_marker(path: &Path) -> Result<UninstallPendingMarker> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str::<UninstallPendingMarker>(&raw)
        .with_context(|| format!("parse {}", path.display()))
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
        let root_dir = match receipt_path.parent() {
            Some(parent) => parent.to_path_buf(),
            None => {
                warn!(
                    target: "stellatune_plugins::discover",
                    receipt = %receipt_path.display(),
                    "skip invalid plugin receipt path"
                );
                continue;
            },
        };
        let uninstall_pending = pending_marker_path_for_plugin_root(&root_dir);
        if uninstall_pending.exists() {
            warn!(
                target: "stellatune_plugins::discover",
                receipt = %receipt_path.display(),
                marker = %uninstall_pending.display(),
                "skip plugin pending uninstall"
            );
            continue;
        }

        let receipt = match read_receipt(&receipt_path) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    target: "stellatune_plugins::discover",
                    receipt = %receipt_path.display(),
                    "skip unreadable plugin receipt: {e:#}"
                );
                continue;
            },
        };

        if receipt.manifest.id.trim().is_empty() {
            warn!(
                target: "stellatune_plugins::discover",
                receipt = %receipt_path.display(),
                "skip plugin receipt with empty manifest.id"
            );
            continue;
        }

        if receipt.library_rel_path.trim().is_empty() {
            warn!(
                target: "stellatune_plugins::discover",
                receipt = %receipt_path.display(),
                plugin_id = %receipt.manifest.id,
                "skip plugin receipt with empty library_rel_path"
            );
            continue;
        }

        out.push(DiscoveredPlugin {
            library_path: root_dir.join(&receipt.library_rel_path),
            root_dir,
            manifest: receipt.manifest,
        });
    }

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
        let Some(root) = marker_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        let mut marker = read_uninstall_pending_marker(&marker_path)
            .unwrap_or_else(|_| default_marker_for_root(&root));
        if let Err(e) = std::fs::remove_dir_all(&root) {
            marker.retry_count = marker.retry_count.saturating_add(1);
            marker.last_error = Some(e.to_string());
            marker.state = if marker.retry_count >= DELETE_FAILED_RETRY_THRESHOLD {
                PluginInstallState::DeleteFailed
            } else {
                PluginInstallState::PendingUninstall
            };
            if let Err(write_err) = write_uninstall_pending_marker(&marker_path, &marker) {
                warn!(
                    target: "stellatune_plugins::discover",
                    marker = %marker_path.display(),
                    "failed to update uninstall marker: {write_err:#}"
                );
            }
            warn!(
                target: "stellatune_plugins::discover",
                root = %root.display(),
                "cleanup pending uninstall failed: {e:#}"
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
        .and_then(|v| v.to_str())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown-plugin".to_string())
}
