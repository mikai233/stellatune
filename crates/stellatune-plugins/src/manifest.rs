use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use stellatune_plugin_protocol::PluginMetadata;
use tracing::warn;

pub const INSTALL_RECEIPT_FILE_NAME: &str = ".install.json";

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
            .unwrap_or(stellatune_plugin_api::STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1)
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
    pub receipt_path: PathBuf,
    pub manifest: PluginManifest,
    pub library_path: PathBuf,
}

pub fn receipt_path_for_plugin_root(root: &Path) -> PathBuf {
    root.join(INSTALL_RECEIPT_FILE_NAME)
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

pub fn discover_plugins(dir: impl AsRef<Path>) -> Result<Vec<DiscoveredPlugin>> {
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
            }
        };

        let receipt = match read_receipt(&receipt_path) {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    target: "stellatune_plugins::discover",
                    receipt = %receipt_path.display(),
                    "skip unreadable plugin receipt: {e:#}"
                );
                continue;
            }
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
            receipt_path,
            manifest: receipt.manifest,
        });
    }

    Ok(out)
}
