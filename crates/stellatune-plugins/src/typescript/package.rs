use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::manifest::{
    TYPESCRIPT_MANIFEST_FILE_NAME, TypeScriptPluginManifest, read_typescript_manifest,
};

pub const TYPESCRIPT_INSTALL_RECEIPT_FILE_NAME: &str = ".install-v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeScriptInstallReceipt {
    pub manifest: TypeScriptPluginManifest,
    pub content_sha256: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct InstalledTypeScriptPlugin {
    pub manifest: TypeScriptPluginManifest,
    pub root_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum TypeScriptPackageError {
    #[error("plugin package I/O failed during {operation}: {message}")]
    Io {
        operation: &'static str,
        message: String,
    },
    #[error("invalid TypeScript plugin package: {0}")]
    Invalid(String),
    #[error(transparent)]
    Manifest(#[from] super::manifest::ManifestV2Error),
}

pub fn install_typescript_artifact(
    plugins_dir: &Path,
    artifact_path: &Path,
) -> Result<InstalledTypeScriptPlugin, TypeScriptPackageError> {
    std::fs::create_dir_all(plugins_dir).map_err(|error| io("create plugins directory", error))?;
    let temp = tempfile::tempdir().map_err(|error| io("create staging directory", error))?;
    let staging = temp.path().join("package");
    if artifact_path.is_dir() {
        crate::package::copy_dir_recursive(artifact_path, &staging)
            .map_err(|error| invalid_error(error.to_string()))?;
    } else if artifact_path.extension().and_then(|item| item.to_str()) == Some("zip") {
        std::fs::create_dir_all(&staging).map_err(|error| io("create ZIP staging", error))?;
        crate::package::extract_zip_to_dir(artifact_path, &staging)
            .map_err(|error| invalid_error(error.to_string()))?;
    } else {
        return invalid("artifact must be a directory or .zip archive");
    }
    let (manifest, package_root) = find_single_manifest(&staging)?;
    let content_sha256 = hash_files(&package_root, false)?;

    let install_root = plugins_dir.join(&manifest.id);
    if install_root.exists()
        && !install_root
            .join(TYPESCRIPT_INSTALL_RECEIPT_FILE_NAME)
            .is_file()
    {
        return invalid(format!(
            "plugin '{}' already exists as a non-v2 installation",
            manifest.id
        ));
    }
    let incoming = unique_sibling(plugins_dir, "incoming", &manifest.id);
    crate::package::copy_dir_recursive(&package_root, &incoming)
        .map_err(|error| invalid_error(error.to_string()))?;
    write_receipt(
        &incoming,
        &TypeScriptInstallReceipt {
            manifest: manifest.clone(),
            content_sha256,
        },
    )?;

    let backup = if install_root.exists() {
        let backup = unique_sibling(plugins_dir, "backup", &manifest.id);
        std::fs::rename(&install_root, &backup)
            .map_err(|error| io("backup existing v2 plugin", error))?;
        Some(backup)
    } else {
        None
    };
    if let Err(error) = std::fs::rename(&incoming, &install_root) {
        if let Some(backup) = &backup {
            let _ = std::fs::rename(backup, &install_root);
        }
        return Err(io("promote v2 plugin", error));
    }
    if let Some(backup) = backup {
        let _ = std::fs::remove_dir_all(backup);
    }
    Ok(InstalledTypeScriptPlugin {
        manifest,
        root_dir: install_root,
    })
}

pub fn discover_typescript_plugins(
    plugins_dir: &Path,
) -> Result<Vec<InstalledTypeScriptPlugin>, TypeScriptPackageError> {
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }
    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(plugins_dir).map_err(|error| io("list plugins", error))? {
        let entry = entry.map_err(|error| io("read plugin directory entry", error))?;
        if !entry
            .file_type()
            .map_err(|error| io("stat plugin", error))?
            .is_dir()
        {
            continue;
        }
        let root = entry.path();
        let receipt_path = root.join(TYPESCRIPT_INSTALL_RECEIPT_FILE_NAME);
        if !receipt_path.is_file() {
            continue;
        }
        let receipt: TypeScriptInstallReceipt = serde_json::from_slice(
            &std::fs::read(&receipt_path).map_err(|error| io("read v2 receipt", error))?,
        )
        .map_err(|error| invalid_error(error.to_string()))?;
        let manifest = read_typescript_manifest(&root.join(TYPESCRIPT_MANIFEST_FILE_NAME))?;
        if manifest != receipt.manifest {
            return invalid(format!(
                "manifest does not match install receipt under {}",
                root.display()
            ));
        }
        if hash_files(&root, true)? != receipt.content_sha256 {
            return invalid(format!(
                "package content hash mismatch under {}",
                root.display()
            ));
        }
        plugins.push(InstalledTypeScriptPlugin {
            manifest,
            root_dir: root,
        });
    }
    plugins.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
    Ok(plugins)
}

/// Removes a v2 plugin. A returned path is a quarantined directory that can be
/// retried at process exit; it is no longer visible to discovery or Registry.
pub fn uninstall_typescript_plugin(
    plugins_dir: &Path,
    plugin_id: &str,
) -> Result<Option<PathBuf>, TypeScriptPackageError> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return invalid("plugin id must not be empty");
    }
    let root = plugins_dir.join(plugin_id);
    if !root.exists() {
        return invalid(format!("plugin '{plugin_id}' is not installed"));
    }
    if !root.join(TYPESCRIPT_INSTALL_RECEIPT_FILE_NAME).is_file() {
        return invalid(format!("plugin '{plugin_id}' is not a v2 installation"));
    }
    match std::fs::remove_dir_all(&root) {
        Ok(()) => Ok(None),
        Err(_) => {
            let quarantine = unique_sibling(plugins_dir, "cleanup", plugin_id);
            std::fs::rename(&root, &quarantine)
                .map_err(|error| io("move plugin to cleanup quarantine", error))?;
            let _ = std::fs::remove_dir_all(&quarantine);
            Ok(quarantine.exists().then_some(quarantine))
        },
    }
}

fn find_single_manifest(
    staging: &Path,
) -> Result<(TypeScriptPluginManifest, PathBuf), TypeScriptPackageError> {
    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(staging).follow_links(false) {
        let entry = entry.map_err(|error| invalid_error(error.to_string()))?;
        if entry.file_type().is_file()
            && entry.file_name().to_str() == Some(TYPESCRIPT_MANIFEST_FILE_NAME)
            && let Ok(manifest) = read_typescript_manifest(entry.path())
        {
            found.push((manifest, entry.path().parent().unwrap().to_path_buf()));
        }
    }
    if found.len() != 1 {
        return invalid(format!(
            "artifact must contain exactly one valid manifest.json; found {}",
            found.len()
        ));
    }
    Ok(found.pop().unwrap())
}

fn hash_files(
    root: &Path,
    exclude_receipt: bool,
) -> Result<BTreeMap<String, String>, TypeScriptPackageError> {
    let mut hashes = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| invalid_error(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(root).unwrap();
        if exclude_receipt
            && relative.as_os_str() == std::ffi::OsStr::new(TYPESCRIPT_INSTALL_RECEIPT_FILE_NAME)
        {
            continue;
        }
        let bytes = std::fs::read(entry.path()).map_err(|error| io("hash package file", error))?;
        hashes.insert(
            relative.to_string_lossy().replace('\\', "/"),
            format!("{:x}", Sha256::digest(bytes)),
        );
    }
    Ok(hashes)
}

fn write_receipt(
    root: &Path,
    receipt: &TypeScriptInstallReceipt,
) -> Result<(), TypeScriptPackageError> {
    let bytes =
        serde_json::to_vec_pretty(receipt).map_err(|error| invalid_error(error.to_string()))?;
    std::fs::write(root.join(TYPESCRIPT_INSTALL_RECEIPT_FILE_NAME), bytes)
        .map_err(|error| io("write v2 receipt", error))
}

fn unique_sibling(parent: &Path, purpose: &str, plugin_id: &str) -> PathBuf {
    parent.join(format!(
        ".{purpose}-{plugin_id}-{}-{}",
        std::process::id(),
        now_ms()
    ))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn io(operation: &'static str, error: std::io::Error) -> TypeScriptPackageError {
    TypeScriptPackageError::Io {
        operation,
        message: error.to_string(),
    }
}

fn invalid_error(message: impl Into<String>) -> TypeScriptPackageError {
    TypeScriptPackageError::Invalid(message.into())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, TypeScriptPackageError> {
    Err(invalid_error(message))
}
