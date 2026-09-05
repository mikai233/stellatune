use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::protocol::CAPABILITY_RPC_PROTOCOL;

pub const TYPESCRIPT_MANIFEST_FILE_NAME: &str = "manifest.json";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypeScriptCapabilityKind {
    SourceResolver,
    LyricsProvider,
    AuthProvider,
    NetworkControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypeScriptExecutionClass {
    Control,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TypeScriptCapabilityManifest {
    pub id: String,
    pub kind: TypeScriptCapabilityKind,
    pub execution_class: TypeScriptExecutionClass,
    pub display_name: String,
    #[serde(default)]
    pub config_schema: Option<String>,
    /// Local containers handled through `resolve-file` and `inspect-file`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TypeScriptRuntimeManifest {
    pub kind: String,
    pub entry: String,
    pub api_version: u32,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TypeScriptUiManifest {
    pub mode: String,
    #[serde(default)]
    pub mobile_support: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TypeScriptPluginManifest {
    pub manifest_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub runtime: TypeScriptRuntimeManifest,
    pub capabilities: Vec<TypeScriptCapabilityManifest>,
    #[serde(default)]
    pub ui: Option<TypeScriptUiManifest>,
}

#[derive(Debug, Error)]
pub enum ManifestV2Error {
    #[error("failed to read manifest {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse manifest {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("invalid manifest v2: {0}")]
    Invalid(String),
}

pub fn read_typescript_manifest(
    manifest_path: &Path,
) -> Result<TypeScriptPluginManifest, ManifestV2Error> {
    let raw = std::fs::read_to_string(manifest_path).map_err(|source| ManifestV2Error::Read {
        path: manifest_path.to_path_buf(),
        source,
    })?;
    let manifest = serde_json::from_str(&raw).map_err(|source| ManifestV2Error::Parse {
        path: manifest_path.to_path_buf(),
        source,
    })?;
    validate_typescript_manifest(
        &manifest,
        manifest_path.parent().unwrap_or_else(|| Path::new(".")),
    )?;
    Ok(manifest)
}

pub fn validate_typescript_manifest(
    manifest: &TypeScriptPluginManifest,
    package_root: &Path,
) -> Result<(), ManifestV2Error> {
    if manifest.manifest_version != 2 {
        return invalid("manifest_version must be 2");
    }
    validate_id("manifest.id", &manifest.id)?;
    validate_nonempty("manifest.name", &manifest.name)?;
    validate_nonempty("manifest.version", &manifest.version)?;
    if manifest.runtime.kind != "typescript" {
        return invalid("runtime.kind must be 'typescript'");
    }
    if manifest.runtime.api_version != 2 {
        return invalid("runtime.api_version must be 2");
    }
    if manifest.runtime.protocol != CAPABILITY_RPC_PROTOCOL {
        return invalid(format!(
            "runtime.protocol must be '{CAPABILITY_RPC_PROTOCOL}'"
        ));
    }
    validate_package_path(
        "runtime.entry",
        &manifest.runtime.entry,
        package_root,
        "mjs",
    )?;
    if manifest.capabilities.is_empty() {
        return invalid("capabilities must not be empty");
    }
    let mut ids = HashSet::new();
    for capability in &manifest.capabilities {
        validate_id("capability.id", &capability.id)?;
        validate_nonempty("capability.display_name", &capability.display_name)?;
        if !ids.insert(capability.id.clone()) {
            return invalid(format!("duplicate capability id '{}'", capability.id));
        }
        if let Some(schema) = &capability.config_schema {
            validate_package_path("capability.config_schema", schema, package_root, "json")?;
        }
        if !capability.local_extensions.is_empty()
            && capability.kind != TypeScriptCapabilityKind::SourceResolver
        {
            return invalid("local_extensions requires a source-resolver capability");
        }
        let mut extensions = HashSet::new();
        for extension in &capability.local_extensions {
            if extension.is_empty()
                || !extension
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                || !extensions.insert(extension)
            {
                return invalid(
                    "local_extensions must contain unique lowercase extensions without dots",
                );
            }
        }
    }
    if let Some(ui) = &manifest.ui
        && ui.mode != "plugin-hosted"
    {
        return invalid("plugin UI requires an updated package with ui.mode = 'plugin-hosted'");
    }
    validate_package_contents(package_root)
}

pub fn validate_package_contents(package_root: &Path) -> Result<(), ManifestV2Error> {
    for entry in walkdir::WalkDir::new(package_root).follow_links(false) {
        let entry = entry.map_err(|error| ManifestV2Error::Invalid(error.to_string()))?;
        let path = entry.path();
        if entry.file_type().is_symlink() {
            return invalid(format!(
                "symbolic links are not allowed: {}",
                path.display()
            ));
        }
        let relative = path.strip_prefix(package_root).unwrap_or(path);
        if relative
            .components()
            .any(|component| component.as_os_str() == "node_modules")
        {
            return invalid("node_modules is not allowed in a plugin package");
        }
        if entry.file_type().is_file() {
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(extension.as_str(), "node" | "wasm" | "wat" | "component") {
                return invalid(format!(
                    "executable or component payload is not allowed: {}",
                    relative.display()
                ));
            }
            if path.file_name().and_then(|value| value.to_str()) == Some("package.json") {
                return invalid(
                    "package.json/install scripts are not allowed in installed bundles",
                );
            }
        }
    }
    Ok(())
}

fn validate_package_path(
    field: &str,
    value: &str,
    root: &Path,
    expected_extension: &str,
) -> Result<(), ManifestV2Error> {
    validate_nonempty(field, value)?;
    let relative = Path::new(value);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return invalid(format!("{field} contains an unsafe path"));
    }
    if relative.extension().and_then(|item| item.to_str()) != Some(expected_extension) {
        return invalid(format!(
            "{field} must reference a .{expected_extension} file"
        ));
    }
    let resolved = root.join(relative);
    if !resolved.is_file() {
        return invalid(format!("{field} does not exist: {}", resolved.display()));
    }
    Ok(())
}

fn validate_id(field: &str, value: &str) -> Result<(), ManifestV2Error> {
    validate_nonempty(field, value)?;
    if value.chars().any(|character| {
        !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'))
    }) {
        return invalid(format!("{field} contains unsupported characters"));
    }
    Ok(())
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), ManifestV2Error> {
    if value.trim().is_empty() {
        return invalid(format!("{field} must not be empty"));
    }
    Ok(())
}

fn invalid<T>(message: impl Into<String>) -> Result<T, ManifestV2Error> {
    Err(ManifestV2Error::Invalid(message.into()))
}
