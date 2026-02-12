#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

mod capabilities;
mod events;
mod load;
mod manifest;
pub mod runtime;
mod service;
mod types;
mod util;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use libloading::{Library, Symbol};
use serde::Serialize;
use stellatune_plugin_api::{
    STELLATUNE_PLUGIN_API_VERSION, STELLATUNE_PLUGIN_ENTRY_SYMBOL, StHostVTable, StPluginEntry,
};
use stellatune_plugin_api::{StLogLevel, StStr};
use stellatune_plugin_protocol::PluginMetadata;
use tracing::{info, warn};

pub use capabilities::*;
pub use events::*;
pub use load::*;
pub use service::*;
pub use types::*;

pub use manifest::{
    DiscoveredPlugin, INSTALL_RECEIPT_FILE_NAME, PluginInstallReceipt, PluginInstallState,
    PluginManifest, UNINSTALL_PENDING_MARKER_FILE_NAME, UninstallPendingMarker,
    discover_pending_uninstalls, discover_plugins, pending_marker_path_for_plugin_root,
    read_receipt, read_uninstall_pending_marker, receipt_path_for_plugin_root, write_receipt,
    write_uninstall_pending_marker,
};

#[derive(Debug, Clone, Serialize)]
pub struct InstalledPluginInfo {
    pub id: String,
    pub name: String,
    pub root_dir: PathBuf,
    pub library_path: PathBuf,
    pub info_json: Option<String>,
    pub install_state: PluginInstallState,
    pub uninstall_retry_count: u32,
    pub uninstall_last_error: Option<String>,
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn metadata_info_json(metadata: Option<&PluginMetadata>) -> Option<String> {
    let meta = metadata?;
    meta.info
        .as_ref()
        .and_then(|value| serde_json::to_string(value).ok())
}

extern "C" fn default_host_log(_: *mut core::ffi::c_void, level: StLogLevel, msg: StStr) {
    let text = unsafe { util::ststr_to_string_lossy(msg) };
    match level {
        StLogLevel::Error => tracing::error!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Warn => tracing::warn!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Info => tracing::info!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Debug => tracing::debug!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Trace => tracing::trace!(target: "stellatune_plugins::plugin", "{text}"),
    }
}

fn default_host_vtable() -> StHostVTable {
    StHostVTable {
        api_version: STELLATUNE_PLUGIN_API_VERSION,
        user_data: core::ptr::null_mut(),
        log_utf8: Some(default_host_log),
        get_runtime_root_utf8: None,
        emit_event_json_utf8: None,
        poll_host_event_json_utf8: None,
        send_control_json_utf8: None,
        free_host_str_utf8: None,
    }
}

fn dynamic_library_ext() -> &'static str {
    match std::env::consts::OS {
        "windows" => "dll",
        "linux" => "so",
        "macos" => "dylib",
        _ => "",
    }
}

fn is_dynamic_library_file(path: &Path) -> bool {
    let ext = dynamic_library_ext();
    if ext.is_empty() {
        return false;
    }
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
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
        .map_err(|e| anyhow!("invalid zip archive: {:?}", e))?;

    for entry in archive.entries() {
        let entry = entry.map_err(|e| anyhow!("zip entry error: {:?}", e))?;
        let filename = entry
            .file_path()
            .try_normalize()
            .map_err(|e| anyhow!("failed to normalize zip path: {:?}", e))?
            .as_ref()
            .to_string();

        let path = Path::new(&filename);
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(anyhow!(
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
            .map_err(|e| anyhow!("failed to get entry data: {:?}", e))?;
        let data = slice_entry.data();

        match entry.compression_method() {
            rawzip::CompressionMethod::Store => {
                std::io::copy(&mut &*data, &mut out)
                    .with_context(|| format!("extract {} to {}", filename, out_path.display()))?;
            }
            rawzip::CompressionMethod::Deflate => {
                let mut decoder = flate2::read::DeflateDecoder::new(data);
                std::io::copy(&mut decoder, &mut out).with_context(|| {
                    format!("extract (deflate) {} to {}", filename, out_path.display())
                })?;
            }
            method => return Err(anyhow!("unsupported compression method: {:?}", method)),
        }
    }
    Ok(())
}

fn inspect_plugin_library_at(path: &Path) -> Result<PluginManifest> {
    let host_vtable = default_host_vtable();

    // SAFETY: loading dynamic libraries and invoking plugin entrypoints is inherently unsafe.
    let lib = unsafe { Library::new(path) }
        .with_context(|| format!("failed to load plugin library from {}", path.display()))?;

    // SAFETY: symbol type matches the current ABI contract.
    let entry: Symbol<StPluginEntry> = unsafe {
        lib.get(STELLATUNE_PLUGIN_ENTRY_SYMBOL.as_bytes())
            .with_context(|| {
                format!(
                    "missing entry symbol `{}` in {}",
                    STELLATUNE_PLUGIN_ENTRY_SYMBOL,
                    path.display()
                )
            })?
    };

    // SAFETY: entrypoint is trusted by ABI contract; null/version checked below.
    let module_ptr = unsafe { (entry)(&host_vtable as *const StHostVTable) };
    if module_ptr.is_null() {
        return Err(anyhow!("plugin `{}` returned null module", path.display()));
    }
    // SAFETY: module pointer remains valid while library is loaded.
    let module = unsafe { *module_ptr };
    if module.api_version != STELLATUNE_PLUGIN_API_VERSION {
        return Err(anyhow!(
            "plugin `{}` api_version mismatch: plugin={}, host={}",
            path.display(),
            module.api_version,
            STELLATUNE_PLUGIN_API_VERSION
        ));
    }

    let metadata_json = unsafe { util::ststr_to_string_lossy((module.metadata_json_utf8)()) };
    let metadata: PluginMetadata = serde_json::from_str(&metadata_json).with_context(|| {
        format!(
            "invalid metadata_json_utf8 for plugin at {}",
            path.display()
        )
    })?;
    if metadata.id.trim().is_empty() {
        return Err(anyhow!("plugin metadata id is empty at {}", path.display()));
    }
    if metadata.api_version != STELLATUNE_PLUGIN_API_VERSION {
        return Err(anyhow!(
            "plugin `{}` metadata api_version mismatch: plugin={}, host={}",
            metadata.id,
            metadata.api_version,
            STELLATUNE_PLUGIN_API_VERSION
        ));
    }

    Ok(PluginManifest {
        id: metadata.id.clone(),
        api_version: metadata.api_version,
        name: Some(metadata.name.clone()),
        entry_symbol: Some(STELLATUNE_PLUGIN_ENTRY_SYMBOL.to_string()),
        metadata: Some(metadata),
    })
}

fn find_plugin_library_candidates(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(8)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if is_dynamic_library_file(path) {
            out.push(path.to_path_buf());
        }
    }
    out
}

pub fn install_plugin_from_artifact(
    plugins_dir: impl AsRef<Path>,
    artifact_path: impl AsRef<Path>,
) -> Result<InstalledPluginInfo> {
    let plugins_dir = plugins_dir.as_ref();
    let artifact_path = artifact_path.as_ref();
    if !artifact_path.exists() {
        return Err(anyhow!("artifact not found: {}", artifact_path.display()));
    }
    std::fs::create_dir_all(plugins_dir)
        .with_context(|| format!("create {}", plugins_dir.display()))?;

    let temp = tempfile::tempdir().context("create plugin install temp dir")?;
    let staging_root = temp.path().join("staging");
    std::fs::create_dir_all(&staging_root)
        .with_context(|| format!("create {}", staging_root.display()))?;

    let ext = artifact_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if ext == "zip" {
        extract_zip_to_dir(artifact_path, &staging_root)?;
    } else if is_dynamic_library_file(artifact_path) {
        let file_name = artifact_path
            .file_name()
            .ok_or_else(|| anyhow!("invalid artifact path: {}", artifact_path.display()))?;
        let dst = staging_root.join(file_name);
        std::fs::copy(artifact_path, &dst)
            .with_context(|| format!("copy {} -> {}", artifact_path.display(), dst.display()))?;
    } else {
        return Err(anyhow!(
            "unsupported plugin artifact: {} (expect .zip or .{})",
            artifact_path.display(),
            dynamic_library_ext()
        ));
    }

    let mut valid = Vec::<(PathBuf, PluginManifest)>::new();
    for candidate in find_plugin_library_candidates(&staging_root) {
        if let Ok(manifest) = inspect_plugin_library_at(&candidate) {
            valid.push((candidate, manifest));
        }
    }

    let (library_path, manifest) = match valid.len() {
        0 => {
            return Err(anyhow!(
                "no valid StellaTune plugin library found in artifact: {}",
                artifact_path.display()
            ));
        }
        1 => valid.pop().expect("single valid plugin"),
        _ => {
            let ids = valid
                .iter()
                .map(|(_, item)| item.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!(
                "artifact contains multiple StellaTune plugins ({ids}); one artifact must contain exactly one plugin"
            ));
        }
    };

    let library_rel_path = library_path
        .strip_prefix(&staging_root)
        .with_context(|| {
            format!(
                "failed to derive library_rel_path from {}",
                library_path.display()
            )
        })?
        .to_path_buf();
    let install_root = plugins_dir.join(&manifest.id);
    if install_root.exists() {
        std::fs::remove_dir_all(&install_root)
            .with_context(|| format!("remove {}", install_root.display()))?;
    }
    copy_dir_recursive(&staging_root, &install_root)?;

    let library_rel_path_string = library_rel_path.to_string_lossy().replace('\\', "/");
    let receipt = PluginInstallReceipt {
        manifest: manifest.clone(),
        library_rel_path: library_rel_path_string,
    };
    write_receipt(&install_root, &receipt)?;

    let installed = InstalledPluginInfo {
        id: manifest.id.clone(),
        name: manifest.name.clone().unwrap_or_else(|| manifest.id.clone()),
        root_dir: install_root.clone(),
        library_path: install_root.join(library_rel_path),
        info_json: metadata_info_json(manifest.metadata.as_ref()),
        install_state: PluginInstallState::Installed,
        uninstall_retry_count: 0,
        uninstall_last_error: None,
    };

    info!(
        target: "stellatune_plugins::install",
        plugin_id = %installed.id,
        plugin_name = %installed.name,
        install_root = %installed.root_dir.display(),
        library_path = %installed.library_path.display(),
        installed_at_ms = now_unix_ms(),
        "plugin installed"
    );
    Ok(installed)
}

pub fn list_installed_plugins(plugins_dir: impl AsRef<Path>) -> Result<Vec<InstalledPluginInfo>> {
    let plugins_dir = plugins_dir.as_ref();
    let mut out = Vec::new();

    for discovered in manifest::discover_plugins(plugins_dir)? {
        out.push(InstalledPluginInfo {
            id: discovered.manifest.id.clone(),
            name: discovered
                .manifest
                .name
                .clone()
                .unwrap_or_else(|| discovered.manifest.id.clone()),
            root_dir: discovered.root_dir.clone(),
            library_path: discovered.library_path.clone(),
            info_json: metadata_info_json(discovered.manifest.metadata.as_ref()),
            install_state: PluginInstallState::Installed,
            uninstall_retry_count: 0,
            uninstall_last_error: None,
        });
    }

    for pending in manifest::discover_pending_uninstalls(plugins_dir)? {
        let receipt = pending.receipt.as_ref();
        let id = receipt
            .map(|value| value.manifest.id.clone())
            .unwrap_or_else(|| pending.marker.plugin_id.clone());
        let name = receipt
            .and_then(|value| value.manifest.name.clone())
            .unwrap_or_else(|| id.clone());
        let library_path = receipt
            .map(|value| pending.root_dir.join(&value.library_rel_path))
            .unwrap_or_else(|| pending.root_dir.clone());
        let info_json =
            receipt.and_then(|value| metadata_info_json(value.manifest.metadata.as_ref()));

        out.push(InstalledPluginInfo {
            id,
            name,
            root_dir: pending.root_dir.clone(),
            library_path,
            info_json,
            install_state: pending.marker.state,
            uninstall_retry_count: pending.marker.retry_count,
            uninstall_last_error: pending.marker.last_error.clone(),
        });
    }

    out.sort_by(|a, b| {
        a.id.cmp(&b.id).then_with(|| {
            a.root_dir
                .to_string_lossy()
                .cmp(&b.root_dir.to_string_lossy())
        })
    });
    Ok(out)
}

pub fn uninstall_plugin(plugins_dir: impl AsRef<Path>, plugin_id: &str) -> Result<()> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }

    let plugins_dir = plugins_dir.as_ref();
    let mut roots = Vec::new();

    for discovered in manifest::discover_plugins(plugins_dir)? {
        if discovered.manifest.id == plugin_id {
            roots.push(discovered.root_dir);
        }
    }

    for pending in manifest::discover_pending_uninstalls(plugins_dir)? {
        let pending_id = pending
            .receipt
            .as_ref()
            .map(|value| value.manifest.id.as_str())
            .unwrap_or(pending.marker.plugin_id.as_str());
        if pending_id == plugin_id {
            roots.push(pending.root_dir);
        }
    }

    if roots.is_empty() {
        return Err(anyhow!("plugin not installed: {plugin_id}"));
    }

    roots.sort();
    roots.dedup();
    for root in roots {
        if !root.exists() {
            continue;
        }
        match std::fs::remove_dir_all(&root) {
            Ok(()) => {}
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
                    target: "stellatune_plugins::uninstall",
                    plugin_id = %plugin_id,
                    root = %root.display(),
                    marker = %marker_path.display(),
                    "plugin uninstall deferred; will retry on next discovery"
                );
            }
        }
    }

    Ok(())
}
