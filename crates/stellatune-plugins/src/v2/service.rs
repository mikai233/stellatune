use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use stellatune_plugin_api::v2::{STELLATUNE_PLUGIN_API_VERSION_V2, StHostVTableV2};
use stellatune_plugin_api::v2::{
    StDecoderInstanceRefV2, StDspInstanceRefV2, StLyricsProviderInstanceRefV2,
    StOutputSinkInstanceRefV2, StSourceCatalogInstanceRefV2,
};

use crate::runtime::{
    CapabilityKind, GenerationGuard, GenerationId, InstanceId, InstanceRegistry,
    InstanceUpdateCoordinator, LifecycleStore,
};
use crate::{default_host_vtable, manifest};

use super::capability_registry::CapabilityRegistry;
use super::load::{
    LoadedModuleCandidateV2, LoadedPluginModuleV2, RuntimeLoadReportV2, RuntimePluginInfoV2,
    load_discovered_plugin_v2,
};
use super::{
    ActivationReport, CapabilityDescriptorInput, CapabilityDescriptorRecord, CapabilityId,
    PluginGenerationInfo, PluginSlotSnapshot,
};
use super::{
    DecoderInstanceV2, DspInstanceV2, InstanceRuntimeCtx, LyricsProviderInstanceV2,
    OutputSinkInstanceV2, SourceCatalogInstanceV2, ststr_from_str,
};

#[derive(Debug)]
struct PluginGenerationEntry {
    info: PluginGenerationInfo,
    _guard: Arc<GenerationGuard>,
}

#[derive(Debug, Default)]
struct PluginSlotState {
    active: Option<Arc<PluginGenerationEntry>>,
    draining: Vec<Arc<PluginGenerationEntry>>,
}

impl PluginSlotState {
    fn activate(&mut self, next: Arc<PluginGenerationEntry>) {
        if let Some(cur) = self.active.take() {
            self.draining.push(cur);
        }
        self.active = Some(next);
    }

    fn deactivate(&mut self) {
        if let Some(cur) = self.active.take() {
            self.draining.push(cur);
        }
    }
}

struct LoadedPluginGenerationV2 {
    generation: GenerationId,
    plugin_name: String,
    loaded: LoadedPluginModuleV2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoderCandidateScoreV2 {
    pub plugin_id: String,
    pub type_id: String,
    pub score: u16,
}

#[derive(Default)]
struct PluginModuleSlotState {
    active: Option<Arc<LoadedPluginGenerationV2>>,
    draining: Vec<Arc<LoadedPluginGenerationV2>>,
}

impl PluginModuleSlotState {
    fn activate(&mut self, next: LoadedPluginGenerationV2) {
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
    host: StHostVTableV2,
    slots: Mutex<HashMap<String, PluginSlotState>>,
    modules: Mutex<HashMap<String, PluginModuleSlotState>>,
    lifecycle: Arc<LifecycleStore>,
    capabilities: Arc<CapabilityRegistry>,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
    next_generation: AtomicU64,
}

impl PluginRuntimeService {
    pub fn new(host: StHostVTableV2) -> Self {
        Self {
            host,
            slots: Mutex::new(HashMap::new()),
            modules: Mutex::new(HashMap::new()),
            lifecycle: Arc::new(LifecycleStore::default()),
            capabilities: Arc::new(CapabilityRegistry::default()),
            instances: Arc::new(InstanceRegistry::default()),
            updates: Arc::new(InstanceUpdateCoordinator::default()),
            next_generation: AtomicU64::new(0),
        }
    }

    pub fn host(&self) -> &StHostVTableV2 {
        &self.host
    }

    pub fn updates(&self) -> &InstanceUpdateCoordinator {
        &self.updates
    }

    pub fn list_active_plugins(&self) -> Vec<RuntimePluginInfoV2> {
        let mut plugin_ids = self.active_plugin_ids();
        plugin_ids.sort();
        let modules = self.modules.lock().ok();
        let mut out = Vec::with_capacity(plugin_ids.len());
        for plugin_id in plugin_ids {
            let Some(generation) = self.active_generation(&plugin_id) else {
                continue;
            };
            let mut info = RuntimePluginInfoV2 {
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

            if let Some(modules) = modules.as_ref()
                && let Some(slot) = modules.get(&plugin_id)
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
        &self,
        plugin_id: &str,
        metadata_json: String,
        capabilities: Vec<CapabilityDescriptorInput>,
    ) -> ActivationReport {
        let generation_id = GenerationId(self.next_generation.fetch_add(1, Ordering::Relaxed) + 1);
        let guard = GenerationGuard::new_active(generation_id);
        self.lifecycle
            .activate_generation(plugin_id, Arc::clone(&guard));

        let generation = Arc::new(PluginGenerationEntry {
            info: PluginGenerationInfo {
                id: generation_id,
                metadata_json,
                activated_at_unix_ms: now_unix_ms(),
            },
            _guard: guard,
        });
        if let Ok(mut slots) = self.slots.lock() {
            slots
                .entry(plugin_id.to_string())
                .or_default()
                .activate(Arc::clone(&generation));
        }

        let registered =
            self.capabilities
                .register_generation(plugin_id, generation_id, capabilities);
        ActivationReport {
            plugin_id: plugin_id.to_string(),
            generation: generation.info.clone(),
            capabilities: registered,
        }
    }

    pub fn active_generation(&self, plugin_id: &str) -> Option<PluginGenerationInfo> {
        let slots = self.slots.lock().ok()?;
        slots
            .get(plugin_id)?
            .active
            .as_ref()
            .map(|g| g.info.clone())
    }

    pub fn slot_snapshot(&self, plugin_id: &str) -> Option<PluginSlotSnapshot> {
        let slots = self.slots.lock().ok()?;
        let slot = slots.get(plugin_id)?;
        Some(PluginSlotSnapshot {
            plugin_id: plugin_id.to_string(),
            active: slot.active.as_ref().map(|g| g.info.clone()),
            draining: slot.draining.iter().map(|g| g.info.clone()).collect(),
        })
    }

    pub fn active_plugin_ids(&self) -> Vec<String> {
        let Ok(slots) = self.slots.lock() else {
            return Vec::new();
        };
        slots
            .iter()
            .filter(|(_, slot)| slot.active.is_some())
            .map(|(plugin_id, _)| plugin_id.clone())
            .collect()
    }

    pub fn list_active_capabilities(&self, plugin_id: &str) -> Vec<CapabilityDescriptorRecord> {
        let generation = self.active_generation(plugin_id).map(|g| g.id);
        let Some(generation) = generation else {
            return Vec::new();
        };
        self.capabilities.list_for_generation(plugin_id, generation)
    }

    pub fn resolve_active_capability(
        &self,
        plugin_id: &str,
        kind: CapabilityKind,
        type_id: &str,
    ) -> Option<CapabilityDescriptorRecord> {
        let generation = self.active_generation(plugin_id).map(|g| g.id)?;
        self.capabilities.find(plugin_id, generation, kind, type_id)
    }

    pub fn create_decoder_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<DecoderInstanceV2> {
        let capability = self
            .resolve_active_capability(plugin_id, CapabilityKind::Decoder, type_id)
            .ok_or_else(|| anyhow!("decoder capability not found: {plugin_id}::{type_id}"))?;
        let module = self
            .active_loaded_module(plugin_id, capability.generation)
            .ok_or_else(|| anyhow!("active loaded module not found for plugin `{plugin_id}`"))?;
        let Some(create) = module.loaded._module.create_decoder_instance else {
            return Err(anyhow!(
                "plugin `{plugin_id}` does not provide decoder factory"
            ));
        };
        let mut raw = StDecoderInstanceRefV2 {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (create)(
            ststr_from_str(type_id),
            ststr_from_str(config_json),
            &mut raw,
        );
        let plugin_free = module.loaded._module.plugin_free;
        super::status_to_result("create_decoder_instance", status, plugin_free)?;
        let instance_id = self
            .register_instance_for_capability(capability.id)
            .map_err(|e| {
                if !raw.handle.is_null() && !raw.vtable.is_null() {
                    unsafe { ((*raw.vtable).destroy)(raw.handle) };
                }
                e
            })?;
        let ctx = self.instance_ctx(instance_id, plugin_free)?;
        DecoderInstanceV2::from_ffi(ctx, raw)
    }

    pub fn decoder_candidates_for_ext(&self, ext_hint: &str) -> Vec<DecoderCandidateScoreV2> {
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
                out.push(DecoderCandidateScoreV2 {
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
    ) -> Result<DspInstanceV2> {
        let capability = self
            .resolve_active_capability(plugin_id, CapabilityKind::Dsp, type_id)
            .ok_or_else(|| anyhow!("dsp capability not found: {plugin_id}::{type_id}"))?;
        let module = self
            .active_loaded_module(plugin_id, capability.generation)
            .ok_or_else(|| anyhow!("active loaded module not found for plugin `{plugin_id}`"))?;
        let Some(create) = module.loaded._module.create_dsp_instance else {
            return Err(anyhow!("plugin `{plugin_id}` does not provide dsp factory"));
        };
        let mut raw = StDspInstanceRefV2 {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (create)(
            ststr_from_str(type_id),
            sample_rate,
            channels,
            ststr_from_str(config_json),
            &mut raw,
        );
        let plugin_free = module.loaded._module.plugin_free;
        super::status_to_result("create_dsp_instance", status, plugin_free)?;
        let instance_id = self
            .register_instance_for_capability(capability.id)
            .map_err(|e| {
                if !raw.handle.is_null() && !raw.vtable.is_null() {
                    unsafe { ((*raw.vtable).destroy)(raw.handle) };
                }
                e
            })?;
        let ctx = self.instance_ctx(instance_id, plugin_free)?;
        DspInstanceV2::from_ffi(ctx, raw)
    }

    pub fn create_source_catalog_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<SourceCatalogInstanceV2> {
        let capability = self
            .resolve_active_capability(plugin_id, CapabilityKind::SourceCatalog, type_id)
            .ok_or_else(|| anyhow!("source capability not found: {plugin_id}::{type_id}"))?;
        let module = self
            .active_loaded_module(plugin_id, capability.generation)
            .ok_or_else(|| anyhow!("active loaded module not found for plugin `{plugin_id}`"))?;
        let Some(create) = module.loaded._module.create_source_catalog_instance else {
            return Err(anyhow!(
                "plugin `{plugin_id}` does not provide source catalog factory"
            ));
        };
        let mut raw = StSourceCatalogInstanceRefV2 {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (create)(
            ststr_from_str(type_id),
            ststr_from_str(config_json),
            &mut raw,
        );
        let plugin_free = module.loaded._module.plugin_free;
        super::status_to_result("create_source_catalog_instance", status, plugin_free)?;
        let instance_id = self
            .register_instance_for_capability(capability.id)
            .map_err(|e| {
                if !raw.handle.is_null() && !raw.vtable.is_null() {
                    unsafe { ((*raw.vtable).destroy)(raw.handle) };
                }
                e
            })?;
        let ctx = self.instance_ctx(instance_id, plugin_free)?;
        SourceCatalogInstanceV2::from_ffi(ctx, raw)
    }

    pub fn create_lyrics_provider_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<LyricsProviderInstanceV2> {
        let capability = self
            .resolve_active_capability(plugin_id, CapabilityKind::LyricsProvider, type_id)
            .ok_or_else(|| anyhow!("lyrics capability not found: {plugin_id}::{type_id}"))?;
        let module = self
            .active_loaded_module(plugin_id, capability.generation)
            .ok_or_else(|| anyhow!("active loaded module not found for plugin `{plugin_id}`"))?;
        let Some(create) = module.loaded._module.create_lyrics_provider_instance else {
            return Err(anyhow!(
                "plugin `{plugin_id}` does not provide lyrics provider factory"
            ));
        };
        let mut raw = StLyricsProviderInstanceRefV2 {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (create)(
            ststr_from_str(type_id),
            ststr_from_str(config_json),
            &mut raw,
        );
        let plugin_free = module.loaded._module.plugin_free;
        super::status_to_result("create_lyrics_provider_instance", status, plugin_free)?;
        let instance_id = self
            .register_instance_for_capability(capability.id)
            .map_err(|e| {
                if !raw.handle.is_null() && !raw.vtable.is_null() {
                    unsafe { ((*raw.vtable).destroy)(raw.handle) };
                }
                e
            })?;
        let ctx = self.instance_ctx(instance_id, plugin_free)?;
        LyricsProviderInstanceV2::from_ffi(ctx, raw)
    }

    pub fn create_output_sink_instance(
        &self,
        plugin_id: &str,
        type_id: &str,
        config_json: &str,
    ) -> Result<OutputSinkInstanceV2> {
        let capability = self
            .resolve_active_capability(plugin_id, CapabilityKind::OutputSink, type_id)
            .ok_or_else(|| anyhow!("output capability not found: {plugin_id}::{type_id}"))?;
        let module = self
            .active_loaded_module(plugin_id, capability.generation)
            .ok_or_else(|| anyhow!("active loaded module not found for plugin `{plugin_id}`"))?;
        let Some(create) = module.loaded._module.create_output_sink_instance else {
            return Err(anyhow!(
                "plugin `{plugin_id}` does not provide output sink factory"
            ));
        };
        let mut raw = StOutputSinkInstanceRefV2 {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (create)(
            ststr_from_str(type_id),
            ststr_from_str(config_json),
            &mut raw,
        );
        let plugin_free = module.loaded._module.plugin_free;
        super::status_to_result("create_output_sink_instance", status, plugin_free)?;
        let instance_id = self
            .register_instance_for_capability(capability.id)
            .map_err(|e| {
                if !raw.handle.is_null() && !raw.vtable.is_null() {
                    unsafe { ((*raw.vtable).destroy)(raw.handle) };
                }
                e
            })?;
        let ctx = self.instance_ctx(instance_id, plugin_free)?;
        OutputSinkInstanceV2::from_ffi(ctx, raw)
    }

    pub fn register_instance_for_capability(
        &self,
        capability_id: CapabilityId,
    ) -> Result<InstanceId> {
        let capability = self
            .capabilities
            .get(capability_id)
            .ok_or_else(|| anyhow!("unknown capability id {}", capability_id.0))?;

        let active_guard = self
            .lifecycle
            .active_generation(&capability.plugin_id)
            .ok_or_else(|| anyhow!("plugin `{}` has no active generation", capability.plugin_id))?;
        if active_guard.id() != capability.generation {
            return Err(anyhow!(
                "capability `{}` belongs to draining generation {:?}, active is {:?}",
                capability.type_id,
                capability.generation,
                active_guard.id()
            ));
        }

        Ok(self.instances.register(
            capability.plugin_id,
            capability.type_id,
            capability.kind,
            active_guard,
        ))
    }

    pub fn unregister_instance(&self, instance_id: InstanceId) {
        let _ = self.instances.remove(instance_id);
    }

    pub fn deactivate_plugin(&self, plugin_id: &str) -> Option<GenerationId> {
        let generation = self.lifecycle.deactivate_plugin(plugin_id)?;
        if let Ok(mut slots) = self.slots.lock()
            && let Some(slot) = slots.get_mut(plugin_id)
        {
            slot.deactivate();
        }
        if let Ok(mut modules) = self.modules.lock()
            && let Some(slot) = modules.get_mut(plugin_id)
        {
            slot.deactivate();
        }
        Some(generation.id())
    }

    pub fn begin_instance_call(&self, instance_id: InstanceId) -> Result<InstanceCallGuard> {
        let record = self
            .instances
            .get(instance_id)
            .ok_or_else(|| anyhow!("unknown instance id {}", instance_id.0))?;
        record.generation.inc_inflight_call();
        Ok(InstanceCallGuard {
            generation: record.generation,
        })
    }

    /// Mark and collect generations that are now safe to unload.
    ///
    /// This removes capability descriptors for those generations.
    pub fn collect_ready_for_unload(&self, plugin_id: &str) -> Vec<GenerationId> {
        let ready = self.lifecycle.collect_ready_for_unload(plugin_id);
        if ready.is_empty() {
            return Vec::new();
        }

        if let Ok(mut slots) = self.slots.lock()
            && let Some(slot) = slots.get_mut(plugin_id)
        {
            let ready_ids: std::collections::HashSet<GenerationId> =
                ready.iter().map(|g| g.id()).collect();
            slot.draining.retain(|g| !ready_ids.contains(&g.info.id));
        }

        let mut out = Vec::with_capacity(ready.len());
        for generation in ready {
            let gid = generation.id();
            self.capabilities.remove_generation(plugin_id, gid);
            out.push(gid);
        }
        if let Ok(mut modules) = self.modules.lock()
            && let Some(slot) = modules.get_mut(plugin_id)
        {
            let ready_ids: std::collections::HashSet<GenerationId> = out.iter().copied().collect();
            slot.draining.retain(|g| !ready_ids.contains(&g.generation));
            if slot.active.is_none() && slot.draining.is_empty() {
                modules.remove(plugin_id);
            }
        }
        out
    }

    pub fn load_dir_additive_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReportV2> {
        let dir = dir.as_ref();
        let mut report = RuntimeLoadReportV2::default();
        for discovered in manifest::discover_plugins(dir)? {
            if disabled_ids.contains(&discovered.manifest.id) {
                continue;
            }
            if self.active_generation(&discovered.manifest.id).is_some() {
                continue;
            }
            match load_discovered_plugin_v2(&discovered, &self.host) {
                Ok(candidate) => {
                    let activated = self.activate_loaded_candidate(candidate);
                    report.unloaded_generations += activated.unloaded_generations;
                    report.loaded.push(activated.info);
                }
                Err(e) => report
                    .errors
                    .push(e.context(format!("while loading plugin `{}`", discovered.manifest.id))),
            }
        }
        Ok(report)
    }

    pub fn reload_dir_filtered(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: &HashSet<String>,
    ) -> Result<RuntimeLoadReportV2> {
        let dir = dir.as_ref();
        let mut report = RuntimeLoadReportV2::default();
        let mut discovered_ids = HashSet::<String>::new();
        for discovered in manifest::discover_plugins(dir)? {
            discovered_ids.insert(discovered.manifest.id.clone());
            if disabled_ids.contains(&discovered.manifest.id) {
                continue;
            }
            match load_discovered_plugin_v2(&discovered, &self.host) {
                Ok(candidate) => {
                    let activated = self.activate_loaded_candidate(candidate);
                    report.unloaded_generations += activated.unloaded_generations;
                    report.loaded.push(activated.info);
                }
                Err(e) => report.errors.push(e.context(format!(
                    "while reloading plugin `{}`",
                    discovered.manifest.id
                ))),
            }
        }

        let mut active_plugin_ids = self.active_plugin_ids();
        active_plugin_ids.sort();
        for plugin_id in active_plugin_ids {
            if disabled_ids.contains(&plugin_id) || !discovered_ids.contains(&plugin_id) {
                if self.deactivate_plugin(&plugin_id).is_some() {
                    report.deactivated.push(plugin_id.clone());
                }
                report.unloaded_generations += self.collect_ready_for_unload(&plugin_id).len();
            }
        }
        Ok(report)
    }

    pub fn unload_plugin(&self, plugin_id: &str) -> RuntimeLoadReportV2 {
        let mut report = RuntimeLoadReportV2::default();
        if self.deactivate_plugin(plugin_id).is_some() {
            report.deactivated.push(plugin_id.to_string());
        }
        report.unloaded_generations += self.collect_ready_for_unload(plugin_id).len();
        report
    }

    fn instance_ctx(
        &self,
        instance_id: InstanceId,
        plugin_free: super::PluginFreeFn,
    ) -> Result<InstanceRuntimeCtx> {
        let record = self
            .instances
            .get(instance_id)
            .ok_or_else(|| anyhow!("unknown instance id {}", instance_id.0))?;
        Ok(InstanceRuntimeCtx {
            instance_id,
            instances: Arc::clone(&self.instances),
            generation: record.generation,
            updates: Arc::clone(&self.updates),
            plugin_free,
        })
    }

    fn active_loaded_module(
        &self,
        plugin_id: &str,
        generation: GenerationId,
    ) -> Option<Arc<LoadedPluginGenerationV2>> {
        let modules = self.modules.lock().ok()?;
        let slot = modules.get(plugin_id)?;
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

    fn activate_loaded_candidate(&self, candidate: LoadedModuleCandidateV2) -> ActivatedLoad {
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
            info: RuntimePluginInfoV2 {
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
        &self,
        plugin_id: &str,
        generation: GenerationId,
        plugin_name: String,
        loaded: LoadedPluginModuleV2,
    ) {
        if let Ok(mut modules) = self.modules.lock() {
            modules
                .entry(plugin_id.to_string())
                .or_default()
                .activate(LoadedPluginGenerationV2 {
                    generation,
                    plugin_name,
                    loaded,
                });
        }
    }
}

pub type SharedPluginRuntimeServiceV2 = Arc<Mutex<PluginRuntimeService>>;

pub fn shared_runtime_service_v2() -> SharedPluginRuntimeServiceV2 {
    static SHARED: OnceLock<SharedPluginRuntimeServiceV2> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            Arc::new(Mutex::new(PluginRuntimeService::new(
                default_host_vtable_v2(),
            )))
        })
        .clone()
}

struct ActivatedLoad {
    info: RuntimePluginInfoV2,
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

fn normalize_ext_hint(raw: impl AsRef<str>) -> String {
    raw.as_ref()
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

fn default_host_vtable_v2() -> StHostVTableV2 {
    let mut host_v1 = default_host_vtable();
    host_v1.api_version = STELLATUNE_PLUGIN_API_VERSION_V2;
    StHostVTableV2 {
        api_version: host_v1.api_version,
        user_data: host_v1.user_data,
        log_utf8: host_v1.log_utf8,
        get_runtime_root_utf8: None,
        emit_event_json_utf8: None,
        poll_host_event_json_utf8: None,
        send_control_json_utf8: None,
        free_host_str_utf8: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_host() -> StHostVTableV2 {
        StHostVTableV2 {
            api_version: STELLATUNE_PLUGIN_API_VERSION_V2,
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
        let svc = PluginRuntimeService::new(test_host());
        let report = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::Decoder, "decoder.a")],
        );
        assert_eq!(report.capabilities.len(), 1);
        let got = svc
            .resolve_active_capability("dev.test.plugin", CapabilityKind::Decoder, "decoder.a")
            .expect("resolve active capability");
        assert_eq!(got.id, report.capabilities[0].id);
    }

    #[test]
    fn draining_generation_not_unloadable_with_live_instance() {
        let svc = PluginRuntimeService::new(test_host());
        let g1 = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::Dsp, "dsp.a")],
        );
        let inst = svc
            .register_instance_for_capability(g1.capabilities[0].id)
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
        let svc = PluginRuntimeService::new(test_host());
        let g1 = svc.activate_generation(
            "dev.test.plugin",
            "{}".to_string(),
            vec![cap(CapabilityKind::OutputSink, "sink.a")],
        );
        let inst = svc
            .register_instance_for_capability(g1.capabilities[0].id)
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
        let svc = PluginRuntimeService::new(test_host());
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
