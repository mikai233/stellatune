use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use libloading::{Library, Symbol};
use stellatune_plugin_api::{
    STELLATUNE_PLUGIN_API_VERSION, STELLATUNE_PLUGIN_ENTRY_SYMBOL, StHostVTable, StPluginEntry,
    StPluginModule,
};
use stellatune_plugin_protocol::PluginMetadata;

use crate::manifest::DiscoveredPlugin;

use super::events::{PluginHostCtx, build_plugin_host_vtable};
use super::{CapabilityDescriptorInput, capability_input_from_ffi};

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
    pub unloaded_generations: usize,
    pub errors: Vec<anyhow::Error>,
}

pub(crate) struct LoadedPluginModule {
    pub(crate) root_dir: PathBuf,
    pub(crate) library_path: PathBuf,
    pub(crate) _shadow_library_path: PathBuf,
    pub(crate) _module: StPluginModule,
    pub(crate) _lib: Library,
    pub(crate) _host_vtable: Box<StHostVTable>,
    pub(crate) _host_ctx: Box<PluginHostCtx>,
}

pub(crate) struct LoadedModuleCandidate {
    pub(crate) plugin_id: String,
    pub(crate) plugin_name: String,
    pub(crate) metadata_json: String,
    pub(crate) root_dir: PathBuf,
    pub(crate) library_path: PathBuf,
    pub(crate) capabilities: Vec<CapabilityDescriptorInput>,
    pub(crate) loaded_module: LoadedPluginModule,
}

pub(crate) fn load_discovered_plugin(
    discovered: &DiscoveredPlugin,
    base_host: &StHostVTable,
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

    // SAFETY: Loading and calling foreign plugin entrypoint is inherently unsafe.
    let lib = unsafe { Library::new(&shadow_library_path) }.with_context(|| {
        format!(
            "failed to load plugin library from shadow copy {} (source: {})",
            shadow_library_path.display(),
            discovered.library_path.display(),
        )
    })?;

    let entry_symbol = discovered
        .manifest
        .entry_symbol
        .as_deref()
        .unwrap_or(STELLATUNE_PLUGIN_ENTRY_SYMBOL);
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

    let (host_vtable, host_ctx) =
        build_plugin_host_vtable(base_host, &discovered.manifest.id, &discovered.root_dir);

    // SAFETY: Plugin entrypoint is trusted by ABI contract. Null and version checked below.
    let module_ptr = unsafe { (entry)(host_vtable.as_ref() as *const StHostVTable) };
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
    let mut capabilities = Vec::with_capacity(cap_count);
    for index in 0..cap_count {
        let desc_ptr = (module.capability_get)(index);
        if desc_ptr.is_null() {
            return Err(anyhow!(
                "plugin `{}` capability_get({index}) returned null",
                metadata.id
            ));
        }
        // SAFETY: Descriptor pointer comes from plugin capability table and is read-only.
        let input = unsafe { capability_input_from_ffi(&*desc_ptr) };
        capabilities.push(input);
    }

    Ok(LoadedModuleCandidate {
        plugin_id: metadata.id,
        plugin_name: metadata.name,
        metadata_json,
        root_dir: discovered.root_dir.clone(),
        library_path: discovered.library_path.clone(),
        capabilities,
        loaded_module: LoadedPluginModule {
            root_dir: discovered.root_dir.clone(),
            library_path: discovered.library_path.clone(),
            _shadow_library_path: shadow_library_path,
            _module: module,
            _lib: lib,
            _host_vtable: host_vtable,
            _host_ctx: host_ctx,
        },
    })
}

fn make_shadow_library_copy(source_library: &Path, plugin_id: &str) -> Result<PathBuf> {
    static SHADOW_COPY_SEQ: AtomicU64 = AtomicU64::new(0);

    let file_name = source_library
        .file_name()
        .ok_or_else(|| anyhow!("invalid plugin library path: {}", source_library.display()))?;
    let stamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let seq = SHADOW_COPY_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let pid = std::process::id();

    let mut safe_plugin_id = plugin_id.trim().to_string();
    if safe_plugin_id.is_empty() {
        safe_plugin_id = "unknown-plugin".to_string();
    }
    safe_plugin_id = safe_plugin_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    let shadow_dir = std::env::temp_dir()
        .join("stellatune")
        .join("plugin-shadow")
        .join(safe_plugin_id);
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
