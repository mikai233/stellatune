use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use libloading::{Library, Symbol};
use stellatune_plugin_api::{
    PluginMetadata, STELLATUNE_PLUGIN_API_VERSION, StHostVTable, StPluginEntry, StPluginModule,
};

use tracing::debug;

use crate::manifest::DiscoveredPlugin;

use super::events::{PluginEventBus, PluginHostCtx, build_plugin_host_vtable};

#[derive(Debug, Clone, Default)]
pub struct RuntimePluginInfo {
    pub id: String,
    pub name: String,
    pub metadata_json: String,
    pub root_dir: Option<PathBuf>,
    pub library_path: Option<PathBuf>,
}

#[derive(Debug, Default)]
pub struct RuntimeLoadReport {
    pub loaded: Vec<RuntimePluginInfo>,
    pub deactivated: Vec<String>,
    pub reclaimed_leases: usize,
    pub errors: Vec<anyhow::Error>,
}

pub(crate) struct LoadedPluginModule {
    pub(crate) root_dir: PathBuf,
    pub(crate) library_path: PathBuf,
    pub(crate) shadow_library_path: PathBuf,
    pub(crate) module: StPluginModule,
    // Keep dynamic library loaded while any lease references this module.
    #[allow(dead_code)]
    pub(crate) library: Library,
    // Keep host callback table/context alive for plugin-owned pointers.
    #[allow(dead_code)]
    pub(crate) host_vtable: Box<StHostVTable>,
    #[allow(dead_code)]
    pub(crate) host_ctx: Box<PluginHostCtx>,
}

pub(crate) struct LoadedModuleCandidate {
    pub(crate) plugin_id: String,
    pub(crate) plugin_name: String,
    pub(crate) metadata_json: String,
    pub(crate) root_dir: PathBuf,
    pub(crate) library_path: PathBuf,
    pub(crate) loaded_module: LoadedPluginModule,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ShadowCleanupReport {
    pub scanned: usize,
    pub deleted: usize,
    pub failed: usize,
    pub skipped_active: usize,
    pub skipped_recent_current_process: usize,
    pub skipped_unrecognized: usize,
}

pub(crate) fn load_discovered_plugin(
    discovered: &DiscoveredPlugin,
    base_host: &StHostVTable,
    event_bus: PluginEventBus,
) -> Result<LoadedModuleCandidate> {
    if discovered.manifest.api_version != STELLATUNE_PLUGIN_API_VERSION {
        return Err(anyhow!(
            "plugin `{}` api_version mismatch: plugin={}, host={}",
            discovered.manifest.id,
            discovered.manifest.api_version,
            STELLATUNE_PLUGIN_API_VERSION
        ));
    }
    if !discovered.library_path.exists() {
        return Err(anyhow!(
            "plugin `{}` library not found: {}",
            discovered.manifest.id,
            discovered.library_path.display()
        ));
    }

    let shadow_library_path =
        make_shadow_library_copy(&discovered.library_path, &discovered.manifest.id)?;

    let load_started = Instant::now();
    // SAFETY: Loading and calling foreign plugin entrypoint is inherently unsafe.
    let lib = unsafe { Library::new(&shadow_library_path) }.with_context(|| {
        format!(
            "failed to load plugin library from shadow copy {} (source: {})",
            shadow_library_path.display(),
            discovered.library_path.display(),
        )
    })?;
    let load_elapsed = load_started.elapsed();
    if load_elapsed.as_millis() > 100 {
        debug!(
            plugin_id = discovered.manifest.id,
            elapsed_ms = load_elapsed.as_millis() as u64,
            "plugin library loaded slowly"
        );
    }

    let entry_symbol = discovered.manifest.entry_symbol();
    // SAFETY: Symbol type matches ABI contract; validated by plugin load checks.
    let entry: Symbol<StPluginEntry> = unsafe {
        lib.get(entry_symbol.as_bytes()).with_context(|| {
            format!(
                "missing entry symbol `{}` in {}",
                entry_symbol,
                shadow_library_path.display()
            )
        })?
    };

    let (host_vtable, host_ctx) = build_plugin_host_vtable(
        base_host,
        &discovered.manifest.id,
        &discovered.root_dir,
        event_bus,
    );

    // SAFETY: Plugin entrypoint is trusted by ABI contract. Null and version checked below.
    let entry_started = Instant::now();
    let module_ptr = unsafe { (entry)(host_vtable.as_ref() as *const StHostVTable) };
    let entry_elapsed = entry_started.elapsed();
    if entry_elapsed.as_millis() > 50 {
        debug!(
            plugin_id = discovered.manifest.id,
            elapsed_ms = entry_elapsed.as_millis() as u64,
            "plugin entrypoint executed slowly"
        );
    }
    if module_ptr.is_null() {
        return Err(anyhow!(
            "plugin `{}` returned null module pointer",
            discovered.manifest.id
        ));
    }
    // SAFETY: Module pointer comes from plugin entrypoint and remains valid while library loaded.
    let module = unsafe { *module_ptr };
    if module.api_version != STELLATUNE_PLUGIN_API_VERSION {
        return Err(anyhow!(
            "plugin `{}` api_version mismatch: plugin={}, host={}",
            discovered.manifest.id,
            module.api_version,
            STELLATUNE_PLUGIN_API_VERSION
        ));
    }

    let metadata_json =
        unsafe { crate::util::ststr_to_string_lossy((module.metadata_json_utf8)()) };
    let metadata: PluginMetadata = serde_json::from_str(&metadata_json).with_context(|| {
        format!(
            "invalid metadata_json_utf8 for plugin `{}` at {}",
            discovered.manifest.id,
            discovered.library_path.display()
        )
    })?;
    if metadata.id != discovered.manifest.id {
        return Err(anyhow!(
            "plugin id mismatch: manifest.id=`{}`, metadata.id=`{}`",
            discovered.manifest.id,
            metadata.id
        ));
    }
    if metadata.api_version != STELLATUNE_PLUGIN_API_VERSION {
        return Err(anyhow!(
            "plugin `{}` metadata api_version mismatch: plugin={}, host={}",
            metadata.id,
            metadata.api_version,
            STELLATUNE_PLUGIN_API_VERSION
        ));
    }

    let cap_count = (module.capability_count)();
    for index in 0..cap_count {
        let desc_ptr = (module.capability_get)(index);
        if desc_ptr.is_null() {
            return Err(anyhow!(
                "plugin `{}` capability_get({index}) returned null",
                metadata.id
            ));
        }
    }

    Ok(LoadedModuleCandidate {
        plugin_id: metadata.id,
        plugin_name: metadata.name,
        metadata_json,
        root_dir: discovered.root_dir.clone(),
        library_path: discovered.library_path.clone(),
        loaded_module: LoadedPluginModule {
            root_dir: discovered.root_dir.clone(),
            library_path: discovered.library_path.clone(),
            shadow_library_path,
            module,
            library: lib,
            host_vtable,
            host_ctx,
        },
    })
}

fn make_shadow_library_copy(source_library: &Path, plugin_id: &str) -> Result<PathBuf> {
    let file_name = source_library
        .file_name()
        .ok_or_else(|| anyhow!("invalid plugin library path: {}", source_library.display()))?;
    let stamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let seq = SHADOW_COPY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let pid = std::process::id();

    let shadow_dir = shadow_root_dir().join(sanitize_plugin_id(plugin_id));
    std::fs::create_dir_all(&shadow_dir)
        .with_context(|| format!("create shadow plugin dir {}", shadow_dir.display()))?;

    let shadow_name = format!("{stamp_ms}-{pid}-{seq}-{}", file_name.to_string_lossy());
    let shadow_path = shadow_dir.join(shadow_name);
    std::fs::copy(source_library, &shadow_path).with_context(|| {
        format!(
            "copy plugin library to shadow path {} -> {}",
            source_library.display(),
            shadow_path.display()
        )
    })?;
    Ok(shadow_path)
}

static SHADOW_COPY_SEQ: AtomicU64 = AtomicU64::new(0);

fn shadow_root_dir() -> PathBuf {
    std::env::temp_dir()
        .join("stellatune")
        .join("plugin-shadow")
}

fn sanitize_plugin_id(plugin_id: &str) -> String {
    let mut safe_plugin_id = plugin_id.trim().to_string();
    if safe_plugin_id.is_empty() {
        safe_plugin_id = "unknown-plugin".to_string();
    }
    safe_plugin_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct ShadowFileKey {
    stamp_ms: u64,
    pid: u32,
    seq: u64,
}

fn parse_shadow_file_key(file_name: &str) -> Option<ShadowFileKey> {
    let mut parts = file_name.splitn(4, '-');
    let stamp_ms = parts.next()?.parse().ok()?;
    let pid = parts.next()?.parse().ok()?;
    let seq = parts.next()?.parse().ok()?;
    let suffix = parts.next()?;
    if suffix.is_empty() {
        return None;
    }
    Some(ShadowFileKey { stamp_ms, pid, seq })
}

pub(crate) fn cleanup_stale_shadow_libraries(
    protected_paths: &HashSet<PathBuf>,
    grace_period: Duration,
    max_deletions: usize,
) -> ShadowCleanupReport {
    cleanup_stale_shadow_libraries_in_dir(
        &shadow_root_dir(),
        protected_paths,
        grace_period,
        max_deletions,
    )
}

fn cleanup_stale_shadow_libraries_in_dir(
    root: &Path,
    protected_paths: &HashSet<PathBuf>,
    grace_period: Duration,
    max_deletions: usize,
) -> ShadowCleanupReport {
    let mut report = ShadowCleanupReport::default();
    let now = SystemTime::now();
    let current_pid = std::process::id();

    let Ok(plugin_dirs) = std::fs::read_dir(root) else {
        return report;
    };

    for plugin_dir in plugin_dirs.flatten() {
        let plugin_dir_path = plugin_dir.path();
        let Ok(file_type) = plugin_dir.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let Ok(files) = std::fs::read_dir(&plugin_dir_path) else {
            continue;
        };
        for file in files.flatten() {
            if report.deleted >= max_deletions {
                return report;
            }

            let path = file.path();
            let Ok(ft) = file.file_type() else {
                continue;
            };
            if !ft.is_file() {
                continue;
            }
            report.scanned = report.scanned.saturating_add(1);

            if protected_paths.contains(&path) {
                report.skipped_active = report.skipped_active.saturating_add(1);
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
                report.skipped_unrecognized = report.skipped_unrecognized.saturating_add(1);
                continue;
            };
            let Some(key) = parse_shadow_file_key(file_name) else {
                report.skipped_unrecognized = report.skipped_unrecognized.saturating_add(1);
                continue;
            };

            let age = file
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default();
            let _ = (key.stamp_ms, key.seq);
            if key.pid == current_pid && age < grace_period {
                report.skipped_recent_current_process =
                    report.skipped_recent_current_process.saturating_add(1);
                continue;
            }

            match std::fs::remove_file(&path) {
                Ok(_) => {
                    report.deleted = report.deleted.saturating_add(1);
                },
                Err(_) => {
                    report.failed = report.failed.saturating_add(1);
                },
            }
        }

        let _ = std::fs::remove_dir(&plugin_dir_path);
    }

    report
}

#[cfg(test)]
#[path = "tests/load_tests.rs"]
mod tests;
