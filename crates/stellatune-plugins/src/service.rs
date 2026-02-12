use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender};
use stellatune_plugin_api::{STELLATUNE_PLUGIN_API_VERSION, StHostVTable};
use stellatune_plugin_api::{
    StDecoderInstanceRef, StDspInstanceRef, StLyricsProviderInstanceRef, StOutputSinkInstanceRef,
    StSourceCatalogInstanceRef,
};
use stellatune_plugin_api::{StLogLevel, StStr};

use crate::manifest;
use crate::runtime::{
    CapabilityKind, GenerationGuard, GenerationId, GenerationState, InstanceId, InstanceRegistry,
    InstanceUpdateCoordinator,
};

use super::load::{
    LoadedModuleCandidate, LoadedPluginModule, RuntimeLoadReport, RuntimePluginInfo,
    cleanup_stale_shadow_libraries, load_discovered_plugin,
};
use super::{
    ActivationReport, CapabilityDescriptorInput, CapabilityDescriptorRecord, PluginGenerationInfo,
    PluginSlotSnapshot,
};
use super::{
    DecoderInstance, DspInstance, InstanceRuntimeCtx, LyricsProviderInstance, OutputSinkInstance,
    SourceCatalogInstance, ststr_from_str,
};

fn destroy_raw_decoder_instance(raw: &mut StDecoderInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe { ((*raw.vtable).destroy)(raw.handle) };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

fn destroy_raw_dsp_instance(raw: &mut StDspInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe { ((*raw.vtable).destroy)(raw.handle) };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

fn destroy_raw_source_catalog_instance(raw: &mut StSourceCatalogInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe { ((*raw.vtable).destroy)(raw.handle) };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

fn destroy_raw_lyrics_provider_instance(raw: &mut StLyricsProviderInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe { ((*raw.vtable).destroy)(raw.handle) };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

fn close_and_destroy_raw_output_sink_instance(raw: &mut StOutputSinkInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe {
        ((*raw.vtable).close)(raw.handle);
        ((*raw.vtable).destroy)(raw.handle);
    };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

impl PreparedCreateContext {
    fn ensure_generation_active(&self) -> Result<()> {
        if self.generation.state() == GenerationState::Active {
            return Ok(());
        }
        Err(anyhow!(
            "instance creation rejected: {}::{:?}::{} is no longer active",
            self.plugin_id,
            self.kind,
            self.type_id
        ))
    }

    fn alloc_instance_ctx(
        &self,
        plugin_free: super::PluginFreeFn,
    ) -> (InstanceId, InstanceRuntimeCtx) {
        let instance_id = self.instances.register(Arc::clone(&self.generation));
        (
            instance_id,
            InstanceRuntimeCtx {
                instance_id,
                instances: Arc::clone(&self.instances),
                generation: Arc::clone(&self.generation),
                owner: super::InstanceCallOwner::new(),
                updates: Arc::clone(&self.updates),
                plugin_free,
            },
        )
    }
}

fn create_decoder_instance_from_context(
    prepared: PreparedCreateContext,
    config_json: &str,
) -> Result<DecoderInstance> {
    prepared.ensure_generation_active()?;
    let Some(create) = prepared.module.loaded._module.create_decoder_instance else {
        return Err(anyhow!(
            "plugin `{}` does not provide decoder factory",
            prepared.plugin_id
        ));
    };
    let mut raw = StDecoderInstanceRef {
        handle: core::ptr::null_mut(),
        vtable: core::ptr::null(),
        reserved0: 0,
        reserved1: 0,
    };
    let status = (create)(
        ststr_from_str(&prepared.type_id),
        ststr_from_str(config_json),
        &mut raw,
    );
    let plugin_free = prepared.module.loaded._module.plugin_free;
    super::status_to_result("create_decoder_instance", status, plugin_free)?;
    let (instance_id, ctx) = prepared.alloc_instance_ctx(plugin_free);
    match DecoderInstance::from_ffi(ctx, raw) {
        Ok(instance) => Ok(instance),
        Err(err) => {
            destroy_raw_decoder_instance(&mut raw);
            let _ = prepared.instances.remove(instance_id);
            Err(err)
        }
    }
}

fn create_dsp_instance_from_context(
    prepared: PreparedCreateContext,
    sample_rate: u32,
    channels: u16,
    config_json: &str,
) -> Result<DspInstance> {
    prepared.ensure_generation_active()?;
    let Some(create) = prepared.module.loaded._module.create_dsp_instance else {
        return Err(anyhow!(
            "plugin `{}` does not provide dsp factory",
            prepared.plugin_id
        ));
    };
    let mut raw = StDspInstanceRef {
        handle: core::ptr::null_mut(),
        vtable: core::ptr::null(),
        reserved0: 0,
        reserved1: 0,
    };
    let status = (create)(
        ststr_from_str(&prepared.type_id),
        sample_rate,
        channels,
        ststr_from_str(config_json),
        &mut raw,
    );
    let plugin_free = prepared.module.loaded._module.plugin_free;
    super::status_to_result("create_dsp_instance", status, plugin_free)?;
    let (instance_id, ctx) = prepared.alloc_instance_ctx(plugin_free);
    match DspInstance::from_ffi(ctx, raw) {
        Ok(instance) => Ok(instance),
        Err(err) => {
            destroy_raw_dsp_instance(&mut raw);
            let _ = prepared.instances.remove(instance_id);
            Err(err)
        }
    }
}

fn create_source_catalog_instance_from_context(
    prepared: PreparedCreateContext,
    config_json: &str,
) -> Result<SourceCatalogInstance> {
    prepared.ensure_generation_active()?;
    let Some(create) = prepared
        .module
        .loaded
        ._module
        .create_source_catalog_instance
    else {
        return Err(anyhow!(
            "plugin `{}` does not provide source catalog factory",
            prepared.plugin_id
        ));
    };
    let mut raw = StSourceCatalogInstanceRef {
        handle: core::ptr::null_mut(),
        vtable: core::ptr::null(),
        reserved0: 0,
        reserved1: 0,
    };
    let status = (create)(
        ststr_from_str(&prepared.type_id),
        ststr_from_str(config_json),
        &mut raw,
    );
    let plugin_free = prepared.module.loaded._module.plugin_free;
    super::status_to_result("create_source_catalog_instance", status, plugin_free)?;
    let (instance_id, ctx) = prepared.alloc_instance_ctx(plugin_free);
    match SourceCatalogInstance::from_ffi(ctx, raw) {
        Ok(instance) => Ok(instance),
        Err(err) => {
            destroy_raw_source_catalog_instance(&mut raw);
            let _ = prepared.instances.remove(instance_id);
            Err(err)
        }
    }
}

fn create_lyrics_provider_instance_from_context(
    prepared: PreparedCreateContext,
    config_json: &str,
) -> Result<LyricsProviderInstance> {
    prepared.ensure_generation_active()?;
    let Some(create) = prepared
        .module
        .loaded
        ._module
        .create_lyrics_provider_instance
    else {
        return Err(anyhow!(
            "plugin `{}` does not provide lyrics provider factory",
            prepared.plugin_id
        ));
    };
    let mut raw = StLyricsProviderInstanceRef {
        handle: core::ptr::null_mut(),
        vtable: core::ptr::null(),
        reserved0: 0,
        reserved1: 0,
    };
    let status = (create)(
        ststr_from_str(&prepared.type_id),
        ststr_from_str(config_json),
        &mut raw,
    );
    let plugin_free = prepared.module.loaded._module.plugin_free;
    super::status_to_result("create_lyrics_provider_instance", status, plugin_free)?;
    let (instance_id, ctx) = prepared.alloc_instance_ctx(plugin_free);
    match LyricsProviderInstance::from_ffi(ctx, raw) {
        Ok(instance) => Ok(instance),
        Err(err) => {
            destroy_raw_lyrics_provider_instance(&mut raw);
            let _ = prepared.instances.remove(instance_id);
            Err(err)
        }
    }
}

fn create_output_sink_instance_from_context(
    prepared: PreparedCreateContext,
    config_json: &str,
) -> Result<OutputSinkInstance> {
    prepared.ensure_generation_active()?;
    let Some(create) = prepared.module.loaded._module.create_output_sink_instance else {
        return Err(anyhow!(
            "plugin `{}` does not provide output sink factory",
            prepared.plugin_id
        ));
    };
    let mut raw = StOutputSinkInstanceRef {
        handle: core::ptr::null_mut(),
        vtable: core::ptr::null(),
        reserved0: 0,
        reserved1: 0,
    };
    let status = (create)(
        ststr_from_str(&prepared.type_id),
        ststr_from_str(config_json),
        &mut raw,
    );
    let plugin_free = prepared.module.loaded._module.plugin_free;
    super::status_to_result("create_output_sink_instance", status, plugin_free)?;
    let (instance_id, ctx) = prepared.alloc_instance_ctx(plugin_free);
    match OutputSinkInstance::from_ffi(ctx, raw) {
        Ok(instance) => Ok(instance),
        Err(err) => {
            close_and_destroy_raw_output_sink_instance(&mut raw);
            let _ = prepared.instances.remove(instance_id);
            Err(err)
        }
    }
}

struct PluginRuntimeMetrics {
    plugin_generations_draining: AtomicU64,
}

impl PluginRuntimeMetrics {
    fn new() -> Self {
        Self {
            plugin_generations_draining: AtomicU64::new(0),
        }
    }

    fn set_draining(&self, draining: usize) -> u64 {
        let draining = draining as u64;
        self.plugin_generations_draining
            .store(draining, Ordering::Relaxed);
        draining
    }
}

fn plugin_runtime_metrics() -> &'static PluginRuntimeMetrics {
    static METRICS: OnceLock<PluginRuntimeMetrics> = OnceLock::new();
    METRICS.get_or_init(PluginRuntimeMetrics::new)
}

fn total_draining_generations(slots: &HashMap<String, PluginSlotState>) -> usize {
    slots.values().map(|slot| slot.draining.len()).sum()
}

fn capability_records_for_generation(
    plugin_id: &str,
    generation: GenerationId,
    inputs: Vec<CapabilityDescriptorInput>,
) -> Vec<CapabilityDescriptorRecord> {
    inputs
        .into_iter()
        .map(|input| CapabilityDescriptorRecord {
            plugin_id: plugin_id.to_string(),
            generation,
            kind: input.kind,
            type_id: input.type_id,
            display_name: input.display_name,
            config_schema_json: input.config_schema_json,
            default_config_json: input.default_config_json,
        })
        .collect()
}

const SHADOW_CLEANUP_GRACE_PERIOD: Duration = Duration::ZERO;
const SHADOW_CLEANUP_MAX_DELETIONS_PER_RUN: usize = 200;

#[derive(Debug)]
struct PluginGenerationEntry {
    info: PluginGenerationInfo,
    guard: Arc<GenerationGuard>,
    capabilities: Vec<CapabilityDescriptorRecord>,
}

#[derive(Debug, Default)]
struct PluginSlotState {
    active: Option<Arc<PluginGenerationEntry>>,
    draining: Vec<Arc<PluginGenerationEntry>>,
}

impl PluginSlotState {
    fn activate(&mut self, next: Arc<PluginGenerationEntry>) {
        if let Some(cur) = self.active.take() {
            cur.guard.mark_draining();
            self.draining.push(cur);
        }
        self.active = Some(next);
    }

    fn deactivate(&mut self) -> Option<Arc<PluginGenerationEntry>> {
        let cur = self.active.take()?;
        cur.guard.mark_draining();
        self.draining.push(Arc::clone(&cur));
        Some(cur)
    }

    fn collect_ready_for_unload(&mut self) -> Vec<Arc<PluginGenerationEntry>> {
        let mut ready = Vec::new();
        let mut i = 0usize;
        while i < self.draining.len() {
            if self.draining[i].guard.can_unload_now() {
                let generation = self.draining.swap_remove(i);
                generation.guard.mark_unloaded();
                ready.push(generation);
            } else {
                i += 1;
            }
        }
        ready
    }
}

struct LoadedPluginGeneration {
    generation: GenerationId,
    plugin_name: String,
    source_fingerprint: SourceLibraryFingerprint,
    loaded: LoadedPluginModule,
}

#[derive(Clone)]
struct PreparedCreateContext {
    plugin_id: String,
    type_id: String,
    kind: CapabilityKind,
    module: Arc<LoadedPluginGeneration>,
    generation: Arc<GenerationGuard>,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceLibraryFingerprint {
    library_path: PathBuf,
    file_size: u64,
    modified_unix_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncMode {
    Additive,
    Reconcile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PluginSyncAction {
    LoadNew { plugin_id: String },
    ReloadChanged { plugin_id: String },
    DeactivateMissingOrDisabled { plugin_id: String },
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSyncPlanSummary {
    pub discovered: usize,
    pub disabled: usize,
    pub actions_total: usize,
    pub load_new: usize,
    pub reload_changed: usize,
    pub deactivate: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeSyncActionOutcome {
    pub action: String,
    pub plugin_id: String,
    pub outcome: String,
}

#[derive(Debug, Default)]
pub struct RuntimeSyncReport {
    pub load_report: RuntimeLoadReport,
    pub plan: RuntimeSyncPlanSummary,
    pub actions: Vec<RuntimeSyncActionOutcome>,
    pub plan_ms: u64,
    pub execute_ms: u64,
    pub total_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderCandidateScore {
    pub plugin_id: String,
    pub type_id: String,
    pub score: u16,
}

#[derive(Default)]
struct PluginModuleSlotState {
    active: Option<Arc<LoadedPluginGeneration>>,
    draining: Vec<Arc<LoadedPluginGeneration>>,
}

impl PluginModuleSlotState {
    fn activate(&mut self, next: LoadedPluginGeneration) {
        if let Some(cur) = self.active.take() {
            self.draining.push(cur);
        }
        self.active = Some(Arc::new(next));
    }

    fn deactivate(&mut self) {
        if let Some(cur) = self.active.take() {
            self.draining.push(cur);
        }
    }
}

pub struct PluginRuntimeService {
    host: StHostVTable,
    slots: HashMap<String, PluginSlotState>,
    modules: HashMap<String, PluginModuleSlotState>,
    disabled_plugin_ids: HashSet<String>,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
    next_generation: u64,
}

impl PluginRuntimeService {
    pub fn new(host: StHostVTable) -> Self {
        Self {
            host,
            slots: HashMap::new(),
            modules: HashMap::new(),
            disabled_plugin_ids: HashSet::new(),
            instances: Arc::new(InstanceRegistry::default()),
            updates: Arc::new(InstanceUpdateCoordinator::default()),
            next_generation: 0,
        }
    }

    pub fn host(&self) -> &StHostVTable {
        &self.host
    }

    pub fn updates(&self) -> &InstanceUpdateCoordinator {
        &self.updates
    }

    pub fn set_disabled_plugin_ids(&mut self, disabled_ids: HashSet<String>) {
        self.disabled_plugin_ids = disabled_ids;
    }

    pub fn set_plugin_enabled(&mut self, plugin_id: &str, enabled: bool) {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return;
        }
        if enabled {
            self.disabled_plugin_ids.remove(plugin_id);
        } else {
            self.disabled_plugin_ids.insert(plugin_id.to_string());
        }
    }

    pub fn disabled_plugin_ids(&self) -> HashSet<String> {
        self.disabled_plugin_ids.clone()
    }

    pub fn list_active_plugins(&self) -> Vec<RuntimePluginInfo> {
        let mut plugin_ids = self.active_plugin_ids();
        plugin_ids.sort();
        let mut out = Vec::with_capacity(plugin_ids.len());
        for plugin_id in plugin_ids {
            let Some(generation) = self.active_generation(&plugin_id) else {
                continue;
            };
            let mut info = RuntimePluginInfo {
                id: plugin_id.clone(),
                name: plugin_id.clone(),
                metadata_json: generation.metadata_json.clone(),
                root_dir: None,
                library_path: None,
            };
            if let Ok(metadata) = serde_json::from_str::<stellatune_plugin_protocol::PluginMetadata>(
                &generation.metadata_json,
            ) {
                info.name = metadata.name;
            }

            if let Some(slot) = self.modules.get(&plugin_id)
                && let Some(active) = slot.active.as_ref()
            {
                info.name = active.plugin_name.clone();
                info.root_dir = Some(active.loaded.root_dir.clone());
                info.library_path = Some(active.loaded.library_path.clone());
            }
            out.push(info);
        }
        out
    }

    pub fn activate_generation(
        &mut self,
        plugin_id: &str,
        metadata_json: String,
        capabilities: Vec<CapabilityDescriptorInput>,
    ) -> ActivationReport {
        self.next_generation = self.next_generation.saturating_add(1);
        let generation_id = GenerationId(self.next_generation);
        let guard = GenerationGuard::new_active(generation_id);
        let capabilities =
            capability_records_for_generation(plugin_id, generation_id, capabilities);

        let generation = Arc::new(PluginGenerationEntry {
            info: PluginGenerationInfo {
                id: generation_id,
                metadata_json,
                activated_at_unix_ms: now_unix_ms(),
            },
            guard,
            capabilities,
        });
        self.slots
            .entry(plugin_id.to_string())
            .or_default()
            .activate(Arc::clone(&generation));
        let draining_total =
            plugin_runtime_metrics().set_draining(total_draining_generations(&self.slots));
        tracing::debug!(
            plugin_id,
            plugin_generation = generation_id.0,
            plugin_generations_draining = draining_total,
            "plugin generation activated"
        );

        ActivationReport {
            plugin_id: plugin_id.to_string(),
            generation: generation.info.clone(),
            capabilities: generation.capabilities.clone(),
        }
    }

    pub fn active_generation(&self, plugin_id: &str) -> Option<PluginGenerationInfo> {
        self.slots
            .get(plugin_id)?
            .active
            .as_ref()
            .map(|g| g.info.clone())
    }

    pub fn slot_snapshot(&self, plugin_id: &str) -> Option<PluginSlotSnapshot> {
        let slot = self.slots.get(plugin_id)?;
        Some(PluginSlotSnapshot {
            plugin_id: plugin_id.to_string(),
            active: slot.active.as_ref().map(|g| g.info.clone()),
            draining: slot.draining.iter().map(|g| g.info.clone()).collect(),
        })
    }

    pub fn active_plugin_ids(&self) -> Vec<String> {
        self.slots
            .iter()
            .filter(|(_, slot)| slot.active.is_some())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect()
    }

    pub fn list_active_capabilities(&self, plugin_id: &str) -> Vec<CapabilityDescriptorRecord> {
        self.slots
            .get(plugin_id)
            .and_then(|slot| slot.active.as_ref())
            .map(|generation| generation.capabilities.clone())
            .unwrap_or_default()
    }

    pub fn resolve_active_capability(
        &self,
        plugin_id: &str,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Option<CapabilityDescriptorRecord> {
        let generation = self.slots.get(plugin_id)?.active.as_ref()?;
        generation
            .capabilities
            .iter()
            .find(|cap| cap.kind == kind && cap.type_id == type_id)
            .cloned()
    }

    fn prepare_create_context(
        &self,
        plugin_id: &str,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Result<PreparedCreateContext> {
        self.ensure_plugin_enabled(plugin_id)?;
        let capability = self
            .resolve_active_capability(plugin_id, kind, type_id)
            .ok_or_else(|| anyhow!("capability not found: {plugin_id}::{kind:?}::{type_id}"))?;
        let module = self
            .active_loaded_module(plugin_id, capability.generation)
            .ok_or_else(|| anyhow!("active loaded module not found for plugin `{plugin_id}`"))?;
        let active_generation = self
            .slots
            .get(plugin_id)
            .and_then(|slot| slot.active.as_ref().map(Arc::clone))
            .ok_or_else(|| anyhow!("plugin `{plugin_id}` has no active generation"))?;
        if active_generation.info.id != capability.generation {
            return Err(anyhow!(
                "capability `{}` belongs to draining generation {:?}, active is {:?}",
                capability.type_id,
                capability.generation,
                active_generation.info.id
            ));
        }
        Ok(PreparedCreateContext {
            plugin_id: capability.plugin_id,
            type_id: capability.type_id,
            kind: capability.kind,
            module,
            generation: Arc::clone(&active_generation.guard),
            instances: Arc::clone(&self.instances),
            updates: Arc::clone(&self.updates),
        })
    }

    pub fn create_decoder_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<DecoderInstance> {
        let prepared = self.prepare_create_context(plugin_id, CapabilityKind::Decoder, type_id)?;
        create_decoder_instance_from_context(prepared, config_json)
    }

    pub fn decoder_candidates_for_ext(&self, ext_hint: &str) -> Vec<DecoderCandidateScore> {
        let ext = normalize_ext_hint(ext_hint);
        if ext.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::new();
        for plugin_id in self.active_plugin_ids() {
            let mut caps = self.list_active_capabilities(&plugin_id);
            caps.sort_by(|a, b| a.type_id.cmp(&b.type_id));
            for cap in caps {
                if cap.kind != CapabilityKind::Decoder {
                    continue;
                }
                let Some(score) =
                    self.decoder_ext_score_for_type(&plugin_id, cap.generation, &cap.type_id, &ext)
                else {
                    continue;
                };
                if score == 0 {
                    continue;
                }
                out.push(DecoderCandidateScore {
                    plugin_id: plugin_id.clone(),
                    type_id: cap.type_id,
                    score,
                });
            }
        }

        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.plugin_id.cmp(&b.plugin_id))
                .then_with(|| a.type_id.cmp(&b.type_id))
        });
        out
    }

    pub fn create_dsp_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        sample_rate: u32,
        channels: u16,
        config_json: &str,
    ) -> Result<DspInstance> {
        let prepared = self.prepare_create_context(plugin_id, CapabilityKind::Dsp, type_id)?;
        create_dsp_instance_from_context(prepared, sample_rate, channels, config_json)
    }

    pub fn create_source_catalog_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<SourceCatalogInstance> {
        let prepared =
            self.prepare_create_context(plugin_id, CapabilityKind::SourceCatalog, type_id)?;
        create_source_catalog_instance_from_context(prepared, config_json)
    }

    pub fn create_lyrics_provider_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<LyricsProviderInstance> {
        let prepared =
            self.prepare_create_context(plugin_id, CapabilityKind::LyricsProvider, type_id)?;
        create_lyrics_provider_instance_from_context(prepared, config_json)
    }

    pub fn create_output_sink_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<OutputSinkInstance> {
        let prepared =
            self.prepare_create_context(plugin_id, CapabilityKind::OutputSink, type_id)?;
        create_output_sink_instance_from_context(prepared, config_json)
    }

    pub fn register_instance_for_capability(
        &self,
        capability: &CapabilityDescriptorRecord,
    ) -> Result<InstanceId> {
        let active_generation = self
            .slots
            .get(&capability.plugin_id)
            .and_then(|slot| {
                slot.active.as_ref().and_then(|generation| {
                    if generation.capabilities.iter().any(|entry| {
                        entry.generation == capability.generation
                            && entry.kind == capability.kind
                            && entry.type_id == capability.type_id
                    }) {
                        Some(Arc::clone(generation))
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| {
                anyhow!(
                    "active capability not found: {}::{:?}::{}",
                    capability.plugin_id,
                    capability.kind,
                    capability.type_id
                )
            })?;
        if active_generation.info.id != capability.generation {
            return Err(anyhow!(
                "capability `{}` belongs to draining generation {:?}, active is {:?}",
                capability.type_id,
                capability.generation,
                active_generation.info.id
            ));
        }

        Ok(self
            .instances
            .register(Arc::clone(&active_generation.guard)))
    }

    pub fn unregister_instance(&self, instance_id: InstanceId) {
        let _ = self.instances.remove(instance_id);
    }

    pub fn deactivate_plugin(&mut self, plugin_id: &str) -> Option<GenerationId> {
        let generation_id = {
            let slot = self.slots.get_mut(plugin_id)?;
            let generation = slot.deactivate()?;
            generation.info.id
        };
        let draining_total =
            plugin_runtime_metrics().set_draining(total_draining_generations(&self.slots));
        tracing::debug!(
            plugin_id,
            deactivated_generation = generation_id.0,
            plugin_generations_draining = draining_total,
            "plugin generation deactivated"
        );

        if let Some(slot) = self.modules.get_mut(plugin_id) {
            slot.deactivate();
        }
        Some(generation_id)
    }

    pub fn begin_instance_call(&self, instance_id: InstanceId) -> Result<InstanceCallGuard> {
        let generation = self
            .instances
            .generation_of(instance_id)
            .ok_or_else(|| anyhow!("unknown instance id {}", instance_id.0))?;
        generation.inc_inflight_call();
        Ok(InstanceCallGuard { generation })
    }

    /// Mark and collect generations that are now safe to unload.
    pub fn collect_ready_for_unload(&mut self, plugin_id: &str) -> Vec<GenerationId> {
        let (ready, ready_len, remove_slot) = {
            let Some(slot) = self.slots.get_mut(plugin_id) else {
                return Vec::new();
            };
            let ready = slot.collect_ready_for_unload();
            if ready.is_empty() {
                return Vec::new();
            }
            let ready_len = ready.len();
            let remove_slot = slot.active.is_none() && slot.draining.is_empty();
            (ready, ready_len, remove_slot)
        };
        if remove_slot {
            self.slots.remove(plugin_id);
        }
        let draining_total =
            plugin_runtime_metrics().set_draining(total_draining_generations(&self.slots));
        tracing::debug!(
            plugin_id,
            unloaded_generations = ready_len,
            plugin_generations_draining = draining_total,
            "plugin draining generations updated after unload collection"
        );

        let mut out = Vec::with_capacity(ready.len());
        for generation in ready {
            out.push(generation.info.id);
        }
        let mut remove_module_slot = false;
        if let Some(slot) = self.modules.get_mut(plugin_id) {
            let ready_ids: std::collections::HashSet<GenerationId> = out.iter().copied().collect();
            slot.draining.retain(|g| !ready_ids.contains(&g.generation));
            remove_module_slot = slot.active.is_none() && slot.draining.is_empty();
        }
        if remove_module_slot {
            self.modules.remove(plugin_id);
        }
        if !out.is_empty() {
            self.cleanup_shadow_copies_best_effort("collect_ready_for_unload");
        }
        out
    }

    pub fn load_dir_additive_filtered(
        &mut self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        self.set_disabled_plugin_ids(disabled_ids.clone());
        self.load_dir_additive_from_state(dir)
    }

    pub fn load_dir_additive_from_state(
        &mut self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeLoadReport> {
        self.sync_dir_from_state_report(dir, SyncMode::Additive)
            .map(|report| report.load_report)
    }

    pub fn reload_dir_filtered(
        &mut self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        self.set_disabled_plugin_ids(disabled_ids.clone());
        self.reload_dir_from_state(dir)
    }

    pub fn reload_dir_from_state(&mut self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        self.sync_dir_from_state_report(dir, SyncMode::Reconcile)
            .map(|report| report.load_report)
    }

    pub fn reload_dir_detailed_from_state(
        &mut self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        self.sync_dir_from_state_report(dir, SyncMode::Reconcile)
    }

    pub fn unload_plugin(&mut self, plugin_id: &str) -> RuntimeLoadReport {
        let mut report = RuntimeLoadReport::default();
        if self.deactivate_plugin(plugin_id).is_some() {
            report.deactivated.push(plugin_id.to_string());
        }
        report.unloaded_generations += self.collect_ready_for_unload(plugin_id).len();
        self.cleanup_shadow_copies_best_effort("unload_plugin:end");
        report
    }

    pub fn shutdown_and_cleanup(&mut self) -> RuntimeLoadReport {
        let mut report = RuntimeLoadReport::default();
        let mut plugin_ids = self.active_plugin_ids();
        plugin_ids.sort();
        for plugin_id in plugin_ids {
            if self.deactivate_plugin(&plugin_id).is_some() {
                report.deactivated.push(plugin_id.clone());
            }
            report.unloaded_generations += self.collect_ready_for_unload(&plugin_id).len();
        }
        self.cleanup_shadow_copies_best_effort("shutdown_and_cleanup");
        report
    }

    pub fn cleanup_shadow_copies_now(&self) {
        self.cleanup_shadow_copies_best_effort("cleanup_shadow_copies_now");
    }

    fn active_loaded_module(
        &self,
        plugin_id: &str,
        generation: GenerationId,
    ) -> Option<Arc<LoadedPluginGeneration>> {
        let slot = self.modules.get(plugin_id)?;
        if let Some(active) = slot.active.as_ref()
            && active.generation == generation
        {
            return Some(Arc::clone(active));
        }
        slot.draining
            .iter()
            .find(|g| g.generation == generation)
            .map(Arc::clone)
    }

    fn decoder_ext_score_for_type(
        &self,
        plugin_id: &str,
        generation: GenerationId,
        type_id: &str,
        ext: &str,
    ) -> Option<u16> {
        let module = self.active_loaded_module(plugin_id, generation)?;
        let count_fn = module.loaded._module.decoder_ext_score_count?;
        let get_fn = module.loaded._module.decoder_ext_score_get?;

        let mut best_exact: Option<u16> = None;
        let mut best_wildcard: Option<u16> = None;
        let count = (count_fn)(ststr_from_str(type_id));
        for idx in 0..count {
            let ptr = (get_fn)(ststr_from_str(type_id), idx);
            if ptr.is_null() {
                continue;
            }
            let rule = unsafe { *ptr };
            let rule_ext =
                normalize_ext_hint(unsafe { crate::util::ststr_to_string_lossy(rule.ext_utf8) });
            if rule_ext.is_empty() {
                continue;
            }
            if rule_ext == "*" {
                best_wildcard = Some(best_wildcard.map_or(rule.score, |v| v.max(rule.score)));
                continue;
            }
            if rule_ext == ext {
                best_exact = Some(best_exact.map_or(rule.score, |v| v.max(rule.score)));
            }
        }

        best_exact.or(best_wildcard)
    }

    fn sync_dir_from_state_report(
        &mut self,
        dir: impl AsRef<Path>,
        mode: SyncMode,
    ) -> Result<RuntimeSyncReport> {
        let total_started = Instant::now();
        let (begin_reason, end_reason) = match mode {
            SyncMode::Additive => (
                "sync_dir_from_state_additive:begin",
                "sync_dir_from_state_additive:end",
            ),
            SyncMode::Reconcile => (
                "sync_dir_from_state_reconcile:begin",
                "sync_dir_from_state_reconcile:end",
            ),
        };
        self.cleanup_shadow_copies_best_effort(begin_reason);

        let dir = dir.as_ref();
        let plan_started = Instant::now();
        let disabled_ids = self.disabled_plugin_ids();
        let discovered_plugins = manifest::discover_plugins(dir)?;
        let plan = self.plan_sync_actions(&discovered_plugins, &disabled_ids, mode);
        let mut plan_summary = RuntimeSyncPlanSummary {
            discovered: discovered_plugins.len(),
            disabled: disabled_ids.len(),
            actions_total: plan.len(),
            ..RuntimeSyncPlanSummary::default()
        };
        for action in &plan {
            match action {
                PluginSyncAction::LoadNew { .. } => plan_summary.load_new += 1,
                PluginSyncAction::ReloadChanged { .. } => plan_summary.reload_changed += 1,
                PluginSyncAction::DeactivateMissingOrDisabled { .. } => {
                    plan_summary.deactivate += 1
                }
            }
        }
        let plan_ms = plan_started.elapsed().as_millis() as u64;
        tracing::debug!(
            mode = ?mode,
            discovered = plan_summary.discovered,
            disabled = plan_summary.disabled,
            actions = plan_summary.actions_total,
            load_new = plan_summary.load_new,
            reload_changed = plan_summary.reload_changed,
            deactivate = plan_summary.deactivate,
            "plugin sync plan prepared"
        );
        let discovered_by_id = discovered_plugins
            .iter()
            .map(|plugin| (plugin.manifest.id.clone(), plugin))
            .collect::<HashMap<_, _>>();

        let execute_started = Instant::now();
        let mut report = RuntimeLoadReport::default();
        let mut action_outcomes = Vec::new();
        for action in plan {
            match action {
                PluginSyncAction::LoadNew { plugin_id } => {
                    let Some(discovered) = discovered_by_id.get(&plugin_id) else {
                        report.errors.push(anyhow!(
                            "planner inconsistency: missing discovered plugin `{plugin_id}`"
                        ));
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "load_new".to_string(),
                            plugin_id,
                            outcome: "planner_missing_discovered".to_string(),
                        });
                        continue;
                    };
                    match load_discovered_plugin(discovered, &self.host) {
                        Ok(candidate) => {
                            let activated = self.activate_loaded_candidate(candidate);
                            report.unloaded_generations += activated.unloaded_generations;
                            report.loaded.push(activated.info);
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "load_new".to_string(),
                                plugin_id,
                                outcome: "loaded".to_string(),
                            });
                        }
                        Err(error) => {
                            report
                                .errors
                                .push(error.context(format!("while loading plugin `{plugin_id}`")));
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "load_new".to_string(),
                                plugin_id,
                                outcome: "error".to_string(),
                            });
                        }
                    }
                }
                PluginSyncAction::ReloadChanged { plugin_id } => {
                    let Some(discovered) = discovered_by_id.get(&plugin_id) else {
                        report.errors.push(anyhow!(
                            "planner inconsistency: missing discovered plugin `{plugin_id}`"
                        ));
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "reload_changed".to_string(),
                            plugin_id,
                            outcome: "planner_missing_discovered".to_string(),
                        });
                        continue;
                    };
                    match load_discovered_plugin(discovered, &self.host) {
                        Ok(candidate) => {
                            let activated = self.activate_loaded_candidate(candidate);
                            report.unloaded_generations += activated.unloaded_generations;
                            report.loaded.push(activated.info);
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "reload_changed".to_string(),
                                plugin_id,
                                outcome: "reloaded".to_string(),
                            });
                        }
                        Err(error) => {
                            report.errors.push(
                                error.context(format!(
                                    "while reloading changed plugin `{plugin_id}`"
                                )),
                            );
                            action_outcomes.push(RuntimeSyncActionOutcome {
                                action: "reload_changed".to_string(),
                                plugin_id,
                                outcome: "error".to_string(),
                            });
                        }
                    }
                }
                PluginSyncAction::DeactivateMissingOrDisabled { plugin_id } => {
                    if self.deactivate_plugin(&plugin_id).is_some() {
                        report.deactivated.push(plugin_id.clone());
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "deactivate".to_string(),
                            plugin_id: plugin_id.clone(),
                            outcome: "deactivated".to_string(),
                        });
                    } else {
                        action_outcomes.push(RuntimeSyncActionOutcome {
                            action: "deactivate".to_string(),
                            plugin_id: plugin_id.clone(),
                            outcome: "already_inactive".to_string(),
                        });
                    }
                    report.unloaded_generations += self.collect_ready_for_unload(&plugin_id).len();
                }
            }
        }
        let execute_ms = execute_started.elapsed().as_millis() as u64;

        self.cleanup_shadow_copies_best_effort(end_reason);
        let total_ms = total_started.elapsed().as_millis() as u64;
        Ok(RuntimeSyncReport {
            load_report: report,
            plan: plan_summary,
            actions: action_outcomes,
            plan_ms,
            execute_ms,
            total_ms,
        })
    }

    fn plan_sync_actions(
        &self,
        discovered_plugins: &[manifest::DiscoveredPlugin],
        disabled_ids: &HashSet<String>,
        mode: SyncMode,
    ) -> Vec<PluginSyncAction> {
        let discovered_ids = discovered_plugins
            .iter()
            .map(|plugin| plugin.manifest.id.clone())
            .collect::<HashSet<_>>();
        let active_ids = self.active_plugin_ids().into_iter().collect::<HashSet<_>>();

        let mut actions = Vec::new();
        for plugin in discovered_plugins {
            let plugin_id = plugin.manifest.id.trim();
            if plugin_id.is_empty() {
                continue;
            }
            let plugin_id = plugin_id.to_string();
            if disabled_ids.contains(&plugin_id) {
                if matches!(mode, SyncMode::Reconcile) && active_ids.contains(&plugin_id) {
                    actions.push(PluginSyncAction::DeactivateMissingOrDisabled { plugin_id });
                }
                continue;
            }

            match mode {
                SyncMode::Additive => {
                    if !active_ids.contains(&plugin_id) {
                        actions.push(PluginSyncAction::LoadNew { plugin_id });
                    }
                }
                SyncMode::Reconcile => {
                    if !active_ids.contains(&plugin_id) {
                        actions.push(PluginSyncAction::LoadNew { plugin_id });
                        continue;
                    }
                    let next_fingerprint = source_fingerprint_for_path(&plugin.library_path);
                    let active_fingerprint = self.active_source_fingerprint(&plugin_id);
                    if active_fingerprint != Some(next_fingerprint) {
                        actions.push(PluginSyncAction::ReloadChanged { plugin_id });
                    }
                }
            }
        }

        if matches!(mode, SyncMode::Reconcile) {
            for plugin_id in active_ids {
                if disabled_ids.contains(&plugin_id) || !discovered_ids.contains(&plugin_id) {
                    actions.push(PluginSyncAction::DeactivateMissingOrDisabled { plugin_id });
                }
            }
        }

        actions
    }

    fn active_source_fingerprint(&self, plugin_id: &str) -> Option<SourceLibraryFingerprint> {
        let slot = self.modules.get(plugin_id)?;
        slot.active
            .as_ref()
            .map(|active| active.source_fingerprint.clone())
    }

    fn activate_loaded_candidate(&mut self, candidate: LoadedModuleCandidate) -> ActivatedLoad {
        let activation = self.activate_generation(
            &candidate.plugin_id,
            candidate.metadata_json.clone(),
            candidate.capabilities,
        );
        self.activate_loaded_module(
            &candidate.plugin_id,
            activation.generation.id,
            candidate.plugin_name.clone(),
            candidate.loaded_module,
        );
        let unloaded_generations = self.collect_ready_for_unload(&candidate.plugin_id).len();
        ActivatedLoad {
            info: RuntimePluginInfo {
                id: candidate.plugin_id,
                name: candidate.plugin_name,
                metadata_json: candidate.metadata_json,
                root_dir: Some(candidate.root_dir),
                library_path: Some(candidate.library_path),
            },
            unloaded_generations,
        }
    }

    fn activate_loaded_module(
        &mut self,
        plugin_id: &str,
        generation: GenerationId,
        plugin_name: String,
        loaded: LoadedPluginModule,
    ) {
        let source_fingerprint = source_fingerprint_for_path(&loaded.library_path);
        self.modules
            .entry(plugin_id.to_string())
            .or_default()
            .activate(LoadedPluginGeneration {
                generation,
                plugin_name,
                source_fingerprint,
                loaded,
            });
    }

    fn ensure_plugin_enabled(&self, plugin_id: &str) -> Result<()> {
        if self.is_plugin_disabled(plugin_id) {
            return Err(anyhow!("plugin disabled: {plugin_id}"));
        }
        Ok(())
    }

    fn is_plugin_disabled(&self, plugin_id: &str) -> bool {
        self.disabled_plugin_ids.contains(plugin_id)
    }

    fn collect_protected_shadow_paths(&self) -> HashSet<std::path::PathBuf> {
        let mut out = HashSet::new();
        for slot in self.modules.values() {
            if let Some(active) = slot.active.as_ref() {
                out.insert(active.loaded._shadow_library_path.clone());
            }
            for draining in &slot.draining {
                out.insert(draining.loaded._shadow_library_path.clone());
            }
        }
        out
    }

    fn cleanup_shadow_copies_best_effort(&self, reason: &str) {
        let protected = self.collect_protected_shadow_paths();
        let report = cleanup_stale_shadow_libraries(
            &protected,
            SHADOW_CLEANUP_GRACE_PERIOD,
            SHADOW_CLEANUP_MAX_DELETIONS_PER_RUN,
        );
        if report.scanned == 0
            && report.deleted == 0
            && report.failed == 0
            && report.skipped_active == 0
            && report.skipped_recent_current_process == 0
            && report.skipped_unrecognized == 0
        {
            return;
        }
        tracing::debug!(
            reason,
            plugin_shadow_scanned = report.scanned,
            plugin_shadow_deleted = report.deleted,
            plugin_shadow_failed = report.failed,
            plugin_shadow_skipped_active = report.skipped_active,
            plugin_shadow_skipped_recent_current_process = report.skipped_recent_current_process,
            plugin_shadow_skipped_unrecognized = report.skipped_unrecognized,
            "plugin shadow cleanup completed"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginRuntimeCommand {
    SetPluginEnabled { plugin_id: String, enabled: bool },
    ReloadDirFromState { dir: String },
    UnloadPlugin { plugin_id: String },
    ShutdownAndCleanup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginRuntimeCommandOutcome {
    SetPluginEnabled {
        plugin_id: String,
        enabled: bool,
    },
    ReloadDirFromState {
        dir: String,
        prev_count: usize,
        loaded_ids: Vec<String>,
        loaded_count: usize,
        deactivated_count: usize,
        unloaded_generations: usize,
        load_errors: Vec<String>,
        fatal_error: Option<String>,
    },
    UnloadPlugin {
        plugin_id: String,
        deactivated: bool,
        unloaded_generations: usize,
        remaining_draining_generations: usize,
        errors: Vec<String>,
    },
    ShutdownAndCleanup {
        deactivated_count: usize,
        unloaded_generations: usize,
        errors: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginRuntimeEvent {
    CommandCompleted {
        request_id: u64,
        owner: Option<String>,
        outcome: PluginRuntimeCommandOutcome,
    },
    PluginEnabledChanged {
        plugin_id: String,
        enabled: bool,
    },
    PluginsReloaded {
        dir: Option<String>,
        loaded: usize,
        deactivated: usize,
        unloaded_generations: usize,
        errors: usize,
    },
    PluginUnloaded {
        plugin_id: String,
        deactivated: bool,
        unloaded_generations: usize,
        remaining_draining_generations: usize,
    },
    RuntimeShutdown {
        deactivated: usize,
        unloaded_generations: usize,
        errors: usize,
    },
}

type RuntimeActorTask = Box<dyn FnOnce(&mut RuntimeActorState) + Send + 'static>;

struct RuntimeActorState {
    service: PluginRuntimeService,
    subscribers: Vec<Sender<PluginRuntimeEvent>>,
    owner_subscribers: HashMap<String, Vec<Sender<PluginRuntimeEvent>>>,
}

impl RuntimeActorState {
    fn new(host: StHostVTable) -> Self {
        Self {
            service: PluginRuntimeService::new(host),
            subscribers: Vec::new(),
            owner_subscribers: HashMap::new(),
        }
    }

    fn emit_global(&mut self, event: PluginRuntimeEvent) {
        self.subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }

    fn emit_to_owner(&mut self, owner: &str, event: PluginRuntimeEvent) {
        let mut should_remove = false;
        if let Some(subscribers) = self.owner_subscribers.get_mut(owner) {
            subscribers.retain(|tx| tx.send(event.clone()).is_ok());
            should_remove = subscribers.is_empty();
        }
        if should_remove {
            self.owner_subscribers.remove(owner);
        }
    }

    fn emit_to_all_owners(&mut self, event: PluginRuntimeEvent) {
        let mut empty_owners = Vec::new();
        for (owner, subscribers) in &mut self.owner_subscribers {
            subscribers.retain(|tx| tx.send(event.clone()).is_ok());
            if subscribers.is_empty() {
                empty_owners.push(owner.clone());
            }
        }
        for owner in empty_owners {
            self.owner_subscribers.remove(&owner);
        }
    }

    fn emit(&mut self, event: PluginRuntimeEvent) {
        self.emit_global(event.clone());
        self.emit_to_all_owners(event);
    }

    fn emit_command_completed(
        &mut self,
        owner: Option<&str>,
        request_id: u64,
        outcome: PluginRuntimeCommandOutcome,
    ) {
        let event = PluginRuntimeEvent::CommandCompleted {
            request_id,
            owner: owner.map(str::to_string),
            outcome,
        };
        self.emit_global(event.clone());
        if let Some(owner) = owner {
            self.emit_to_owner(owner, event);
        } else {
            self.emit_to_all_owners(event);
        }
    }

    fn execute_command(&mut self, command: PluginRuntimeCommand) -> PluginRuntimeCommandOutcome {
        match command {
            PluginRuntimeCommand::SetPluginEnabled { plugin_id, enabled } => {
                self.service.set_plugin_enabled(&plugin_id, enabled);
                self.emit(PluginRuntimeEvent::PluginEnabledChanged {
                    plugin_id: plugin_id.clone(),
                    enabled,
                });
                PluginRuntimeCommandOutcome::SetPluginEnabled { plugin_id, enabled }
            }
            PluginRuntimeCommand::ReloadDirFromState { dir } => {
                let prev_count = self.service.active_plugin_ids().len();
                match self.service.reload_dir_from_state(&dir) {
                    Ok(report) => {
                        let loaded_ids = report
                            .loaded
                            .iter()
                            .map(|plugin| plugin.id.clone())
                            .collect();
                        let loaded_count = report.loaded.len();
                        let deactivated_count = report.deactivated.len();
                        let unloaded_generations = report.unloaded_generations;
                        let load_errors = report
                            .errors
                            .iter()
                            .map(|err| format!("{err:#}"))
                            .collect::<Vec<_>>();
                        self.emit(PluginRuntimeEvent::PluginsReloaded {
                            dir: Some(dir.clone()),
                            loaded: loaded_count,
                            deactivated: deactivated_count,
                            unloaded_generations,
                            errors: load_errors.len(),
                        });
                        PluginRuntimeCommandOutcome::ReloadDirFromState {
                            dir,
                            prev_count,
                            loaded_ids,
                            loaded_count,
                            deactivated_count,
                            unloaded_generations,
                            load_errors,
                            fatal_error: None,
                        }
                    }
                    Err(err) => PluginRuntimeCommandOutcome::ReloadDirFromState {
                        dir,
                        prev_count,
                        loaded_ids: Vec::new(),
                        loaded_count: 0,
                        deactivated_count: 0,
                        unloaded_generations: 0,
                        load_errors: Vec::new(),
                        fatal_error: Some(err.to_string()),
                    },
                }
            }
            PluginRuntimeCommand::UnloadPlugin { plugin_id } => {
                let report = self.service.unload_plugin(&plugin_id);
                let remaining = self
                    .service
                    .slot_snapshot(&plugin_id)
                    .map(|slot| slot.draining.len())
                    .unwrap_or(0);
                let errors = report
                    .errors
                    .iter()
                    .map(|err| format!("{err:#}"))
                    .collect::<Vec<_>>();
                self.emit(PluginRuntimeEvent::PluginUnloaded {
                    plugin_id: plugin_id.clone(),
                    deactivated: !report.deactivated.is_empty(),
                    unloaded_generations: report.unloaded_generations,
                    remaining_draining_generations: remaining,
                });
                PluginRuntimeCommandOutcome::UnloadPlugin {
                    plugin_id,
                    deactivated: !report.deactivated.is_empty(),
                    unloaded_generations: report.unloaded_generations,
                    remaining_draining_generations: remaining,
                    errors,
                }
            }
            PluginRuntimeCommand::ShutdownAndCleanup => {
                let report = self.service.shutdown_and_cleanup();
                let errors = report
                    .errors
                    .iter()
                    .map(|err| format!("{err:#}"))
                    .collect::<Vec<_>>();
                self.emit(PluginRuntimeEvent::RuntimeShutdown {
                    deactivated: report.deactivated.len(),
                    unloaded_generations: report.unloaded_generations,
                    errors: errors.len(),
                });
                PluginRuntimeCommandOutcome::ShutdownAndCleanup {
                    deactivated_count: report.deactivated.len(),
                    unloaded_generations: report.unloaded_generations,
                    errors,
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct PluginRuntimeHandle {
    tx: Sender<RuntimeActorTask>,
    next_request_id: Arc<AtomicU64>,
}

impl PluginRuntimeHandle {
    fn spawn(host: StHostVTable) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<RuntimeActorTask>();
        thread::Builder::new()
            .name("stellatune-plugin-runtime".to_string())
            .spawn(move || run_plugin_runtime_actor(rx, host))
            .expect("failed to spawn plugin runtime actor");
        Self {
            tx,
            next_request_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn exec_value<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Option<R> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        let task: RuntimeActorTask = Box::new(move |state| {
            let out = f(state);
            let _ = resp_tx.send(out);
        });
        self.tx.send(task).ok()?;
        resp_rx.recv().ok()
    }

    fn exec_result<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut RuntimeActorState) -> R + Send + 'static,
    ) -> Result<R> {
        self.exec_value(f)
            .ok_or_else(|| anyhow!("plugin runtime actor unavailable"))
    }

    pub fn register_runtime_event_sender(&self, sender: Sender<PluginRuntimeEvent>) -> bool {
        self.exec_value(move |state| {
            state.subscribers.push(sender);
        })
        .is_some()
    }

    pub fn register_owner_runtime_event_sender(
        &self,
        owner: &str,
        sender: Sender<PluginRuntimeEvent>,
    ) -> bool {
        let owner = owner.trim().to_string();
        if owner.is_empty() {
            return false;
        }
        self.exec_value(move |state| {
            state
                .owner_subscribers
                .entry(owner)
                .or_default()
                .push(sender);
        })
        .is_some()
    }

    pub fn subscribe_runtime_events(&self) -> Receiver<PluginRuntimeEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let _ = self.register_runtime_event_sender(tx);
        rx
    }

    pub fn subscribe_owner_runtime_events(&self, owner: &str) -> Receiver<PluginRuntimeEvent> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let _ = self.register_owner_runtime_event_sender(owner, tx);
        rx
    }

    pub fn send_command(
        &self,
        owner: Option<String>,
        command: PluginRuntimeCommand,
    ) -> Result<u64> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let owner = owner.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let task: RuntimeActorTask = Box::new(move |state| {
            let outcome = state.execute_command(command);
            state.emit_command_completed(owner.as_deref(), request_id, outcome);
        });
        self.tx
            .send(task)
            .map_err(|_| anyhow!("plugin runtime actor unavailable"))?;
        Ok(request_id)
    }

    pub fn set_disabled_plugin_ids(&self, disabled_ids: HashSet<String>) {
        let _ = self.exec_value(move |state| {
            state.service.set_disabled_plugin_ids(disabled_ids);
        });
    }

    pub fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) {
        let plugin_id = plugin_id.trim().to_string();
        if plugin_id.is_empty() {
            return;
        }
        let _ = self.exec_value(move |state| {
            state.service.set_plugin_enabled(&plugin_id, enabled);
            state.emit(PluginRuntimeEvent::PluginEnabledChanged { plugin_id, enabled });
        });
    }

    pub fn disabled_plugin_ids(&self) -> HashSet<String> {
        self.exec_value(|state| state.service.disabled_plugin_ids())
            .unwrap_or_default()
    }

    pub fn list_active_plugins(&self) -> Vec<RuntimePluginInfo> {
        self.exec_value(|state| state.service.list_active_plugins())
            .unwrap_or_default()
    }

    pub fn active_generation(&self, plugin_id: &str) -> Option<PluginGenerationInfo> {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| state.service.active_generation(&plugin_id))
            .flatten()
    }

    pub fn slot_snapshot(&self, plugin_id: &str) -> Option<PluginSlotSnapshot> {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| state.service.slot_snapshot(&plugin_id))
            .flatten()
    }

    pub fn active_plugin_ids(&self) -> Vec<String> {
        self.exec_value(|state| state.service.active_plugin_ids())
            .unwrap_or_default()
    }

    pub fn list_active_capabilities(&self, plugin_id: &str) -> Vec<CapabilityDescriptorRecord> {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| state.service.list_active_capabilities(&plugin_id))
            .unwrap_or_default()
    }

    pub fn resolve_active_capability(
        &self,
        plugin_id: &str,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Option<CapabilityDescriptorRecord> {
        let plugin_id = plugin_id.to_string();
        let type_id = type_id.to_string();
        self.exec_value(move |state| {
            state
                .service
                .resolve_active_capability(&plugin_id, kind, &type_id)
        })
        .flatten()
    }

    pub fn create_decoder_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<DecoderInstance> {
        let plugin_id = plugin_id.to_string();
        let type_id = type_id.to_string();
        let prepared = self.exec_result(move |state| {
            state
                .service
                .prepare_create_context(&plugin_id, CapabilityKind::Decoder, &type_id)
        })??;
        create_decoder_instance_from_context(prepared, config_json)
    }

    pub fn decoder_candidates_for_ext(&self, ext_hint: &str) -> Vec<DecoderCandidateScore> {
        let ext_hint = ext_hint.to_string();
        self.exec_value(move |state| state.service.decoder_candidates_for_ext(&ext_hint))
            .unwrap_or_default()
    }

    pub fn create_dsp_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        sample_rate: u32,
        channels: u16,
        config_json: &str,
    ) -> Result<DspInstance> {
        let plugin_id = plugin_id.to_string();
        let type_id = type_id.to_string();
        let prepared = self.exec_result(move |state| {
            state
                .service
                .prepare_create_context(&plugin_id, CapabilityKind::Dsp, &type_id)
        })??;
        create_dsp_instance_from_context(prepared, sample_rate, channels, config_json)
    }

    pub fn create_source_catalog_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<SourceCatalogInstance> {
        let plugin_id = plugin_id.to_string();
        let type_id = type_id.to_string();
        let prepared = self.exec_result(move |state| {
            state.service.prepare_create_context(
                &plugin_id,
                CapabilityKind::SourceCatalog,
                &type_id,
            )
        })??;
        create_source_catalog_instance_from_context(prepared, config_json)
    }

    pub fn create_lyrics_provider_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<LyricsProviderInstance> {
        let plugin_id = plugin_id.to_string();
        let type_id = type_id.to_string();
        let prepared = self.exec_result(move |state| {
            state.service.prepare_create_context(
                &plugin_id,
                CapabilityKind::LyricsProvider,
                &type_id,
            )
        })??;
        create_lyrics_provider_instance_from_context(prepared, config_json)
    }

    pub fn create_output_sink_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<OutputSinkInstance> {
        let plugin_id = plugin_id.to_string();
        let type_id = type_id.to_string();
        let prepared = self.exec_result(move |state| {
            state
                .service
                .prepare_create_context(&plugin_id, CapabilityKind::OutputSink, &type_id)
        })??;
        create_output_sink_instance_from_context(prepared, config_json)
    }

    pub fn deactivate_plugin(&self, plugin_id: &str) -> Option<GenerationId> {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| state.service.deactivate_plugin(&plugin_id))
            .flatten()
    }

    pub fn collect_ready_for_unload(&self, plugin_id: &str) -> Vec<GenerationId> {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| state.service.collect_ready_for_unload(&plugin_id))
            .unwrap_or_default()
    }

    pub fn load_dir_additive_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        self.exec_result(move |state| state.service.load_dir_additive_from_state(&dir))?
    }

    pub fn load_dir_additive_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        self.exec_result(move |state| {
            state
                .service
                .load_dir_additive_filtered(&dir, &disabled_ids)
        })?
    }

    pub fn reload_dir_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let disabled_ids = disabled_ids.clone();
        let dir_for_event = dir_path.to_string_lossy().to_string();
        self.exec_result(move |state| {
            let report = state.service.reload_dir_filtered(&dir_path, &disabled_ids);
            if let Ok(success) = &report {
                state.emit(PluginRuntimeEvent::PluginsReloaded {
                    dir: Some(dir_for_event),
                    loaded: success.loaded.len(),
                    deactivated: success.deactivated.len(),
                    unloaded_generations: success.unloaded_generations,
                    errors: success.errors.len(),
                });
            }
            report
        })?
    }

    pub fn reload_dir_from_state(&self, dir: impl AsRef<Path>) -> Result<RuntimeLoadReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let dir_for_event = dir_path.to_string_lossy().to_string();
        self.exec_result(move |state| {
            let report = state.service.reload_dir_from_state(&dir_path);
            if let Ok(success) = &report {
                state.emit(PluginRuntimeEvent::PluginsReloaded {
                    dir: Some(dir_for_event),
                    loaded: success.loaded.len(),
                    deactivated: success.deactivated.len(),
                    unloaded_generations: success.unloaded_generations,
                    errors: success.errors.len(),
                });
            }
            report
        })?
    }

    pub fn reload_dir_detailed_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        let dir_path = dir.as_ref().to_path_buf();
        let dir_for_event = dir_path.to_string_lossy().to_string();
        self.exec_result(move |state| {
            let report = state.service.reload_dir_detailed_from_state(&dir_path);
            if let Ok(success) = &report {
                state.emit(PluginRuntimeEvent::PluginsReloaded {
                    dir: Some(dir_for_event),
                    loaded: success.load_report.loaded.len(),
                    deactivated: success.load_report.deactivated.len(),
                    unloaded_generations: success.load_report.unloaded_generations,
                    errors: success.load_report.errors.len(),
                });
            }
            report
        })?
    }

    pub fn unload_plugin(&self, plugin_id: &str) -> RuntimeLoadReport {
        let plugin_id = plugin_id.to_string();
        self.exec_value(move |state| {
            let report = state.service.unload_plugin(&plugin_id);
            let remaining = state
                .service
                .slot_snapshot(&plugin_id)
                .map(|slot| slot.draining.len())
                .unwrap_or(0);
            state.emit(PluginRuntimeEvent::PluginUnloaded {
                plugin_id,
                deactivated: !report.deactivated.is_empty(),
                unloaded_generations: report.unloaded_generations,
                remaining_draining_generations: remaining,
            });
            report
        })
        .unwrap_or_default()
    }

    pub fn shutdown_and_cleanup(&self) -> RuntimeLoadReport {
        self.exec_value(|state| {
            let report = state.service.shutdown_and_cleanup();
            state.emit(PluginRuntimeEvent::RuntimeShutdown {
                deactivated: report.deactivated.len(),
                unloaded_generations: report.unloaded_generations,
                errors: report.errors.len(),
            });
            report
        })
        .unwrap_or_default()
    }

    pub fn cleanup_shadow_copies_now(&self) {
        let _ = self.exec_value(|state| {
            state.service.cleanup_shadow_copies_now();
        });
    }
}

fn run_plugin_runtime_actor(rx: Receiver<RuntimeActorTask>, host: StHostVTable) {
    let mut state = RuntimeActorState::new(host);
    while let Ok(task) = rx.recv() {
        task(&mut state);
    }
}

pub type SharedPluginRuntimeService = PluginRuntimeHandle;

pub fn shared_runtime_service() -> SharedPluginRuntimeService {
    static SHARED: OnceLock<SharedPluginRuntimeService> = OnceLock::new();
    SHARED
        .get_or_init(|| PluginRuntimeHandle::spawn(default_host_vtable()))
        .clone()
}

struct ActivatedLoad {
    info: RuntimePluginInfo,
    unloaded_generations: usize,
}

pub struct InstanceCallGuard {
    generation: Arc<GenerationGuard>,
}

impl Drop for InstanceCallGuard {
    fn drop(&mut self) {
        self.generation.dec_inflight_call();
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn source_fingerprint_for_path(path: &Path) -> SourceLibraryFingerprint {
    let mut file_size = 0;
    let mut modified_unix_ms = 0;
    if let Ok(meta) = std::fs::metadata(path) {
        file_size = meta.len();
        if let Ok(modified) = meta.modified() {
            modified_unix_ms = modified
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
        }
    }
    SourceLibraryFingerprint {
        library_path: path.to_path_buf(),
        file_size,
        modified_unix_ms,
    }
}

fn normalize_ext_hint(raw: impl AsRef<str>) -> String {
    raw.as_ref()
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

fn default_host_vtable() -> StHostVTable {
    extern "C" fn default_host_log(_: *mut core::ffi::c_void, level: StLogLevel, msg: StStr) {
        let text = unsafe { crate::util::ststr_to_string_lossy(msg) };
        match level {
            StLogLevel::Error => tracing::error!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Warn => tracing::warn!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Info => tracing::info!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Debug => tracing::debug!(target: "stellatune_plugins::plugin", "{text}"),
            StLogLevel::Trace => tracing::trace!(target: "stellatune_plugins::plugin", "{text}"),
        }
    }

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

#[cfg(test)]
mod tests {
    use super::{
        CapabilityDescriptorInput, CapabilityKind, PluginRuntimeService,
        STELLATUNE_PLUGIN_API_VERSION, StHostVTable,
    };

    fn test_host() -> StHostVTable {
        StHostVTable {
            api_version: STELLATUNE_PLUGIN_API_VERSION,
            user_data: core::ptr::null_mut(),
            log_utf8: None,
            get_runtime_root_utf8: None,
            emit_event_json_utf8: None,
            poll_host_event_json_utf8: None,
            send_control_json_utf8: None,
            free_host_str_utf8: None,
        }
    }

    fn cap(kind: CapabilityKind, type_id: &str) -> CapabilityDescriptorInput {
        CapabilityDescriptorInput {
            kind,
            type_id: type_id.to_string(),
            display_name: type_id.to_string(),
            config_schema_json: "{}".to_string(),
            default_config_json: "{}".to_string(),
        }
    }

    #[test]
    fn activate_and_resolve_active_capability() {
        let mut svc = PluginRuntimeService::new(test_host());
        let report = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::Decoder, "decoder.a")],
        );
        assert_eq!(report.capabilities.len(), 1);
        let got = svc
            .resolve_active_capability("dev.test.plugin", CapabilityKind::Decoder, "decoder.a")
            .expect("resolve active capability");
        assert_eq!(got.generation, report.capabilities[0].generation);
        assert_eq!(got.type_id, report.capabilities[0].type_id);
    }

    #[test]
    fn draining_generation_not_unloadable_with_live_instance() {
        let mut svc = PluginRuntimeService::new(test_host());
        let g1 = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::Dsp, "dsp.a")],
        );
        let inst = svc
            .register_instance_for_capability(&g1.capabilities[0])
            .expect("register instance");

        let _g2 = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::Dsp, "dsp.a")],
        );

        let ready0 = svc.collect_ready_for_unload("dev.test.plugin");
        assert!(ready0.is_empty(), "should not unload with live instance");

        svc.unregister_instance(inst);
        let ready1 = svc.collect_ready_for_unload("dev.test.plugin");
        assert_eq!(ready1, vec![g1.generation.id]);
    }

    #[test]
    fn inflight_call_blocks_unload_until_guard_dropped() {
        let mut svc = PluginRuntimeService::new(test_host());
        let g1 = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::OutputSink, "sink.a")],
        );
        let inst = svc
            .register_instance_for_capability(&g1.capabilities[0])
            .expect("register instance");

        let call = svc.begin_instance_call(inst).expect("begin call");
        svc.unregister_instance(inst);
        let _g2 = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::OutputSink, "sink.a")],
        );

        let ready0 = svc.collect_ready_for_unload("dev.test.plugin");
        assert!(ready0.is_empty(), "inflight call should block unload");
        drop(call);
        let ready1 = svc.collect_ready_for_unload("dev.test.plugin");
        assert_eq!(ready1, vec![g1.generation.id]);
    }

    #[test]
    fn deactivate_moves_active_to_draining() {
        let mut svc = PluginRuntimeService::new(test_host());
        let g1 = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::Decoder, "decoder.a")],
        );
        assert!(svc.active_generation("dev.test.plugin").is_some());
        let deactivated = svc
            .deactivate_plugin("dev.test.plugin")
            .expect("deactivate active generation");
        assert_eq!(deactivated, g1.generation.id);
        assert!(svc.active_generation("dev.test.plugin").is_none());
        let ready = svc.collect_ready_for_unload("dev.test.plugin");
        assert_eq!(ready, vec![g1.generation.id]);
    }
}
