use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub api_version: u32,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub entry_symbol: Option<String>,

    #[serde(default)]
    pub library: PluginLibraryPaths,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PluginLibraryPaths {
    #[serde(default)]
    pub windows: Option<String>,
    #[serde(default)]
    pub linux: Option<String>,
    #[serde(default)]
    pub macos: Option<String>,
}

impl PluginManifest {
    pub fn entry_symbol(&self) -> &str {
        self.entry_symbol
            .as_deref()
            .unwrap_or(stellatune_plugin_api::STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1)
    }

    pub fn library_path_for_current_platform(&self) -> Result<&str> {
        match std::env::consts::OS {
            "windows" => self
                .library
                .windows
                .as_deref()
                .ok_or_else(|| anyhow!("missing `[library].windows` in plugin.toml")),
            "linux" => self
                .library
                .linux
                .as_deref()
                .ok_or_else(|| anyhow!("missing `[library].linux` in plugin.toml")),
            "macos" => self
                .library
                .macos
                .as_deref()
                .ok_or_else(|| anyhow!("missing `[library].macos` in plugin.toml")),
            other => Err(anyhow!("unsupported OS for native plugins: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub root_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: PluginManifest,
}

pub fn discover_plugins(dir: impl AsRef<Path>) -> Result<Vec<DiscoveredPlugin>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy() != "plugin.toml" {
            continue;
        }

        let manifest_path = entry.path().to_path_buf();
        let root_dir = manifest_path
            .parent()
            .context("plugin.toml has no parent dir")?
            .to_path_buf();

        let text = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;

        let manifest: PluginManifest = toml::from_str(&text)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

        out.push(DiscoveredPlugin {
            root_dir,
            manifest_path,
            manifest,
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_discovers_plugin_manifest() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_root = tmp.path().join("my_plugin");
        std::fs::create_dir_all(&plugin_root).expect("mkdir");

        let manifest_path = plugin_root.join("plugin.toml");
        std::fs::write(
            &manifest_path,
            r#"
id = "com.example.test"
api_version = 1

[library]
windows = "bin/win64/test.dll"
linux = "bin/linux64/libtest.so"
macos = "bin/macos/libtest.dylib"
"#,
        )
        .expect("write manifest");

        let discovered = discover_plugins(tmp.path()).expect("discover");
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].manifest.id, "com.example.test");

        let lib_rel = discovered[0]
            .manifest
            .library_path_for_current_platform()
            .expect("platform lib path");
        assert!(!lib_rel.is_empty());
    }
}
