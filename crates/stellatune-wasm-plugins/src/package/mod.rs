use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{ErrorContext, Result};
use serde::Serialize;
use tracing::{info, warn};

use crate::manifest::{
    INSTALL_RECEIPT_FILE_NAME, PLUGIN_MANIFEST_FILE_NAME, PluginInstallReceipt, PluginInstallState,
    UninstallPendingMarker, WasmPluginManifest, discover_pending_uninstalls, discover_plugins,
    pending_marker_path_for_plugin_root, read_manifest, validate_manifest, write_receipt,
    write_uninstall_pending_marker,
};

#[derive(Debug, Clone, Serialize)]
pub struct InstalledPlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub root_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub component_count: usize,
    pub install_state: PluginInstallState,
    pub uninstall_retry_count: u32,
    pub uninstall_last_error: Option<String>,
}

pub fn install_from_artifact(
    plugins_dir: impl AsRef<Path>,
    artifact_path: impl AsRef<Path>,
) -> Result<InstalledPlugin> {
    let plugins_dir = plugins_dir.as_ref();
    let artifact_path = artifact_path.as_ref();
    if !artifact_path.exists() {
        return Err(crate::op_error!(
            "artifact not found: {}",
            artifact_path.display()
        ));
    }
    std::fs::create_dir_all(plugins_dir)
        .with_context(|| format!("create {}", plugins_dir.display()))?;

    let temp = tempfile::tempdir().context("create plugin install temp dir")?;
    let staging_root = temp.path().join("staging");
    std::fs::create_dir_all(&staging_root)
        .with_context(|| format!("create {}", staging_root.display()))?;

    if artifact_path.is_dir() {
        copy_dir_recursive(artifact_path, &staging_root)?;
    } else {
        let ext = artifact_path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if ext != "zip" {
            return Err(crate::op_error!(
                "unsupported plugin artifact: {} (expect directory or .zip)",
                artifact_path.display()
            ));
        }
        extract_zip_to_dir(artifact_path, &staging_root)?;
    }

    let (mut valid, invalid) = find_manifest_candidates(&staging_root);
    let (manifest_path, manifest) = match valid.len() {
        0 => {
            if !invalid.is_empty() {
                let details = invalid
                    .iter()
                    .take(3)
                    .map(|(path, error)| format!("{} => {}", path.display(), error))
                    .collect::<Vec<_>>()
                    .join(" | ");
                return Err(crate::op_error!(
                    "no valid wasm plugin manifest found in artifact: {}; invalid manifest candidates: {}",
                    artifact_path.display(),
                    details
                ));
            }
            return Err(crate::op_error!(
                "no valid wasm plugin manifest found in artifact: {}",
                artifact_path.display()
            ));
        },
        1 => valid.pop().expect("single manifest"),
        _ => {
            let ids = valid
                .iter()
                .map(|(_, m)| m.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(crate::op_error!(
                "artifact contains multiple plugin manifests ({ids}); one artifact must contain exactly one plugin"
            ));
        },
    };
    let package_root = manifest_path
        .parent()
        .ok_or_else(|| crate::op_error!("invalid manifest path: {}", manifest_path.display()))?;
    validate_manifest(&manifest, package_root)?;

    let install_root = plugins_dir.join(&manifest.id);
    if install_root.exists() {
        std::fs::remove_dir_all(&install_root)
            .with_context(|| format!("remove {}", install_root.display()))?;
    }
    copy_dir_recursive(package_root, &install_root)?;

    let receipt = PluginInstallReceipt {
        manifest: manifest.clone(),
        manifest_rel_path: PLUGIN_MANIFEST_FILE_NAME.to_string(),
    };
    write_receipt(&install_root, &receipt)?;

    let info = InstalledPlugin {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        root_dir: install_root.clone(),
        manifest_path: install_root.join(PLUGIN_MANIFEST_FILE_NAME),
        component_count: manifest.components.len(),
        install_state: PluginInstallState::Installed,
        uninstall_retry_count: 0,
        uninstall_last_error: None,
    };
    info!(
        target: "stellatune_wasm_plugins::install",
        plugin_id = %info.id,
        plugin_name = %info.name,
        version = %info.version,
        root_dir = %info.root_dir.display(),
        manifest = %info.manifest_path.display(),
        "wasm plugin installed"
    );
    Ok(info)
}

pub fn list_installed(plugins_dir: impl AsRef<Path>) -> Result<Vec<InstalledPlugin>> {
    let plugins_dir = plugins_dir.as_ref();
    let mut out = Vec::new();

    for discovered in discover_plugins(plugins_dir)? {
        out.push(InstalledPlugin {
            id: discovered.manifest.id.clone(),
            name: discovered.manifest.name.clone(),
            version: discovered.manifest.version.clone(),
            root_dir: discovered.root_dir.clone(),
            manifest_path: discovered.manifest_path.clone(),
            component_count: discovered.manifest.components.len(),
            install_state: PluginInstallState::Installed,
            uninstall_retry_count: 0,
            uninstall_last_error: None,
        });
    }

    for pending in discover_pending_uninstalls(plugins_dir)? {
        let manifest = pending.receipt.as_ref().map(|v| &v.manifest);
        out.push(InstalledPlugin {
            id: manifest
                .map(|m| m.id.clone())
                .unwrap_or_else(|| pending.marker.plugin_id.clone()),
            name: manifest
                .map(|m| m.name.clone())
                .unwrap_or_else(|| pending.marker.plugin_id.clone()),
            version: manifest
                .map(|m| m.version.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            root_dir: pending.root_dir.clone(),
            manifest_path: pending
                .receipt
                .as_ref()
                .map(|v| pending.root_dir.join(&v.manifest_rel_path))
                .unwrap_or_else(|| pending.root_dir.join(PLUGIN_MANIFEST_FILE_NAME)),
            component_count: manifest.map(|m| m.components.len()).unwrap_or(0),
            install_state: pending.marker.state,
            uninstall_retry_count: pending.marker.retry_count,
            uninstall_last_error: pending.marker.last_error.clone(),
        });
    }

    out.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.root_dir.cmp(&b.root_dir)));
    Ok(out)
}

pub fn uninstall_by_id(plugins_dir: impl AsRef<Path>, plugin_id: &str) -> Result<()> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Err(crate::op_error!("plugin_id is empty"));
    }
    let plugins_dir = plugins_dir.as_ref();

    let mut roots = Vec::<PathBuf>::new();
    for discovered in discover_plugins(plugins_dir)? {
        if discovered.manifest.id == plugin_id {
            roots.push(discovered.root_dir);
        }
    }
    for pending in discover_pending_uninstalls(plugins_dir)? {
        let pid = pending
            .receipt
            .as_ref()
            .map(|v| v.manifest.id.as_str())
            .unwrap_or(pending.marker.plugin_id.as_str());
        if pid == plugin_id {
            roots.push(pending.root_dir);
        }
    }
    if roots.is_empty() {
        return Err(crate::op_error!("plugin not installed: {plugin_id}"));
    }
    roots.sort();
    roots.dedup();

    for root in roots {
        if !root.exists() {
            continue;
        }
        match std::fs::remove_dir_all(&root) {
            Ok(()) => {},
            Err(remove_err) => {
                let marker_path = pending_marker_path_for_plugin_root(&root);
                let marker = UninstallPendingMarker {
                    plugin_id: plugin_id.to_string(),
                    queued_at_ms: now_unix_ms(),
                    retry_count: 0,
                    last_error: Some(remove_err.to_string()),
                    state: PluginInstallState::PendingUninstall,
                };
                write_uninstall_pending_marker(&marker_path, &marker)?;
                warn!(
                    target: "stellatune_wasm_plugins::uninstall",
                    plugin_id = %plugin_id,
                    root = %root.display(),
                    marker = %marker_path.display(),
                    "wasm plugin uninstall deferred; will retry on next discovery"
                );
            },
        }
    }
    Ok(())
}

pub fn install_receipt_file_name() -> &'static str {
    INSTALL_RECEIPT_FILE_NAME
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("create {}", dst.display()))?;
    for entry in walkdir::WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path == src {
            continue;
        }
        let rel = path
            .strip_prefix(src)
            .with_context(|| format!("strip prefix {} from {}", src.display(), path.display()))?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("create {}", target.display()))?;
            continue;
        }
        if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            std::fs::copy(path, &target)
                .with_context(|| format!("copy {} -> {}", path.display(), target.display()))?;
        }
    }
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, out_dir: &Path) -> Result<()> {
    let buf = std::fs::read(zip_path).with_context(|| format!("read {}", zip_path.display()))?;
    let archive = rawzip::ZipArchive::from_slice(&buf)
        .map_err(|e| crate::op_error!("invalid zip archive: {:?}", e))?;

    for entry in archive.entries() {
        let entry = entry.map_err(|e| crate::op_error!("zip entry error: {:?}", e))?;
        let filename = entry
            .file_path()
            .try_normalize()
            .map_err(|e| crate::op_error!("failed to normalize zip path: {:?}", e))?
            .as_ref()
            .to_string();

        let path = Path::new(&filename);
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(crate::op_error!(
                "unsupported or malicious path in zip: {}",
                filename
            ));
        }

        let out_path = out_dir.join(&filename);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .with_context(|| format!("create {}", out_path.display()))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }

        let mut out = std::fs::File::create(&out_path)
            .with_context(|| format!("create {}", out_path.display()))?;
        let wayfinder = entry.wayfinder();
        let slice_entry = archive
            .get_entry(wayfinder)
            .map_err(|e| crate::op_error!("failed to get entry data: {:?}", e))?;
        let data = slice_entry.data();
        match entry.compression_method() {
            rawzip::CompressionMethod::Store => {
                std::io::copy(&mut &*data, &mut out)
                    .with_context(|| format!("extract {} to {}", filename, out_path.display()))?;
            },
            rawzip::CompressionMethod::Deflate => {
                let mut decoder = flate2::read::DeflateDecoder::new(data);
                std::io::copy(&mut decoder, &mut out).with_context(|| {
                    format!("extract (deflate) {} to {}", filename, out_path.display())
                })?;
            },
            method => {
                return Err(crate::op_error!(
                    "unsupported compression method: {:?}",
                    method
                ));
            },
        }
    }
    Ok(())
}

type ValidManifestCandidate = (PathBuf, WasmPluginManifest);
type InvalidManifestCandidate = (PathBuf, String);

fn find_manifest_candidates(
    root: &Path,
) -> (Vec<ValidManifestCandidate>, Vec<InvalidManifestCandidate>) {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(8)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy() != PLUGIN_MANIFEST_FILE_NAME {
            continue;
        }
        let path = entry.path().to_path_buf();
        match read_manifest(&path) {
            Ok(manifest) => valid.push((path, manifest)),
            Err(error) => invalid.push((path, format!("{error:#}"))),
        }
    }
    (valid, invalid)
}
