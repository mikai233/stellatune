use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use crate::error::Result;
use parking_lot::RwLock;
use tracing::warn;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Cache, Config, Engine, Store};
use wasmtime_wasi::p2::add_to_linker_sync;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder};

use stellatune_host_bindings as wasm_host;
use stellatune_host_bindings::generated::decoder_plugin::DecoderPlugin as DecoderPluginBinding;
use stellatune_host_bindings::generated::decoder_plugin::exports::stellatune::plugin::lifecycle as decoder_lifecycle;
use stellatune_host_bindings::generated::dsp_plugin::DspPlugin as DspPluginBinding;
use stellatune_host_bindings::generated::dsp_plugin::exports::stellatune::plugin::lifecycle as dsp_lifecycle;
use stellatune_host_bindings::generated::lyrics_plugin::LyricsPlugin as LyricsPluginBinding;
use stellatune_host_bindings::generated::lyrics_plugin::exports::stellatune::plugin::lifecycle as lyrics_lifecycle;
use stellatune_host_bindings::generated::output_sink_plugin::OutputSinkPlugin as OutputSinkPluginBinding;
use stellatune_host_bindings::generated::output_sink_plugin::exports::stellatune::plugin::lifecycle as output_sink_lifecycle;
use stellatune_host_bindings::generated::source_plugin::SourcePlugin as SourcePluginBinding;
use stellatune_host_bindings::generated::source_plugin::exports::stellatune::plugin::lifecycle as source_lifecycle;

use crate::host::http::HttpClientHost;
use crate::host::sidecar::{SidecarHost, default_sidecar_host};
use crate::host::stream::HostStreamService;
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    PluginDisableReason, RuntimeCapabilityDescriptor, RuntimePluginDirective, RuntimePluginInfo,
};

pub mod plugin_cell;
use plugin_cell::PluginCell;
pub mod plugin_instance;
mod sidecar_state;
use sidecar_state::SidecarState;

pub trait WasmPluginController: Send + Sync {
    fn install_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
    ) -> Result<()>;
    fn uninstall_plugin(&self, plugin_id: &str, reason: PluginDisableReason) -> Result<()>;
    fn dispatch_directive(&self, plugin_id: &str, directive: RuntimePluginDirective) -> Result<()>;
    fn shutdown(&self) -> Result<()>;
}

mod stores;
use stores::decoder::DecoderStoreData;
use stores::dsp::DspStoreData;
use stores::lyrics::LyricsStoreData;
use stores::output_sink::OutputSinkStoreData;
use stores::source::SourceStoreData;

#[derive(Default)]
struct DirectiveRegistry {
    active_plugins: BTreeSet<String>,
    senders: BTreeMap<String, Vec<Sender<RuntimePluginDirective>>>,
}

#[derive(Clone)]
struct ActivePluginRecord {
    info: RuntimePluginInfo,
    capabilities: Vec<RuntimeCapabilityDescriptor>,
}

pub struct WasmtimePluginController {
    engine: Engine,
    http_client: Arc<dyn HttpClientHost>,
    stream_service: Arc<dyn HostStreamService>,
    sidecar_host: Arc<dyn SidecarHost>,
    directives: RwLock<DirectiveRegistry>,
    plugins: RwLock<BTreeMap<String, ActivePluginRecord>>,
    component_cache: RwLock<BTreeMap<PathBuf, Component>>,
    decoder_linker: Linker<DecoderStoreData>,
    source_linker: Linker<SourceStoreData>,
    lyrics_linker: Linker<LyricsStoreData>,
    output_sink_linker: Linker<OutputSinkStoreData>,
    dsp_linker: Linker<DspStoreData>,
}

impl WasmtimePluginController {
    pub fn new(
        http_client: Arc<dyn HttpClientHost>,
        stream_service: Arc<dyn HostStreamService>,
    ) -> Result<Self> {
        Self::new_with_sidecar_host(http_client, stream_service, default_sidecar_host())
    }

    pub(crate) fn new_with_sidecar_host(
        http_client: Arc<dyn HttpClientHost>,
        stream_service: Arc<dyn HostStreamService>,
        sidecar_host: Arc<dyn SidecarHost>,
    ) -> Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        match Cache::from_file(None) {
            Ok(cache) => {
                config.cache(Some(cache));
            },
            Err(error) => {
                warn!(
                    "wasmtime cache config unavailable; continuing without disk cache: {error:#}"
                );
            },
        };
        let engine = Engine::new(&config)?;

        let mut decoder_linker: Linker<DecoderStoreData> = Linker::new(&engine);
        DecoderPluginBinding::add_to_linker::<_, HasSelf<DecoderStoreData>>(
            &mut decoder_linker,
            |state| state,
        )?;
        add_to_linker_sync(&mut decoder_linker)?;

        let mut source_linker: Linker<SourceStoreData> = Linker::new(&engine);
        SourcePluginBinding::add_to_linker::<_, HasSelf<SourceStoreData>>(
            &mut source_linker,
            |state| state,
        )?;
        add_to_linker_sync(&mut source_linker)?;

        let mut lyrics_linker: Linker<LyricsStoreData> = Linker::new(&engine);
        LyricsPluginBinding::add_to_linker::<_, HasSelf<LyricsStoreData>>(
            &mut lyrics_linker,
            |state| state,
        )?;
        add_to_linker_sync(&mut lyrics_linker)?;

        let mut output_sink_linker: Linker<OutputSinkStoreData> = Linker::new(&engine);
        OutputSinkPluginBinding::add_to_linker::<_, HasSelf<OutputSinkStoreData>>(
            &mut output_sink_linker,
            |state| state,
        )?;
        add_to_linker_sync(&mut output_sink_linker)?;

        let mut dsp_linker: Linker<DspStoreData> = Linker::new(&engine);
        DspPluginBinding::add_to_linker::<_, HasSelf<DspStoreData>>(&mut dsp_linker, |state| {
            state
        })?;
        add_to_linker_sync(&mut dsp_linker)?;

        Ok(Self {
            engine,
            http_client,
            stream_service,
            sidecar_host,
            directives: RwLock::new(DirectiveRegistry::default()),
            plugins: RwLock::new(BTreeMap::new()),
            component_cache: RwLock::new(BTreeMap::new()),
            decoder_linker,
            source_linker,
            lyrics_linker,
            output_sink_linker,
            dsp_linker,
        })
    }

    pub fn shared(
        http_client: Arc<dyn HttpClientHost>,
        stream_service: Arc<dyn HostStreamService>,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(http_client, stream_service)?))
    }

    fn new_decoder_store_data(&self, plugin_root: &Path) -> DecoderStoreData {
        let (wasi_ctx, wasi_table) = create_store_wasi_state();
        DecoderStoreData {
            stream_service: self.stream_service.clone(),
            next_rep: 1,
            streams: BTreeMap::new(),
            sidecar: SidecarState::new(self.sidecar_host.clone()),
            plugin_root: plugin_root.to_path_buf(),
            wasi_ctx,
            wasi_table,
        }
    }

    fn new_source_store_data(&self, plugin_root: &Path) -> SourceStoreData {
        let (wasi_ctx, wasi_table) = create_store_wasi_state();
        SourceStoreData {
            stream_service: self.stream_service.clone(),
            next_rep: 1,
            streams: BTreeMap::new(),
            sidecar: SidecarState::new(self.sidecar_host.clone()),
            plugin_root: plugin_root.to_path_buf(),
            wasi_ctx,
            wasi_table,
        }
    }

    fn new_lyrics_store_data(&self, plugin_root: &Path) -> LyricsStoreData {
        let (wasi_ctx, wasi_table) = create_store_wasi_state();
        LyricsStoreData {
            http_client: self.http_client.clone(),
            sidecar: SidecarState::new(self.sidecar_host.clone()),
            plugin_root: plugin_root.to_path_buf(),
            wasi_ctx,
            wasi_table,
        }
    }

    fn new_output_sink_store_data(&self, plugin_root: &Path) -> OutputSinkStoreData {
        let (wasi_ctx, wasi_table) = create_store_wasi_state();
        OutputSinkStoreData {
            sidecar: SidecarState::new(self.sidecar_host.clone()),
            plugin_root: plugin_root.to_path_buf(),
            wasi_ctx,
            wasi_table,
        }
    }

    fn new_dsp_store_data(&self, plugin_root: &Path) -> DspStoreData {
        let (wasi_ctx, wasi_table) = create_store_wasi_state();
        DspStoreData {
            sidecar: SidecarState::new(self.sidecar_host.clone()),
            plugin_root: plugin_root.to_path_buf(),
            wasi_ctx,
            wasi_table,
        }
    }

    fn ensure_plugin_active(&self, plugin_id: &str) -> Result<()> {
        let routes = self.directives.read();
        if routes.active_plugins.contains(plugin_id) {
            return Ok(());
        }
        Err(crate::op_error!("plugin `{plugin_id}` is not active"))
    }

    fn register_directive_sender(
        &self,
        plugin_id: &str,
        sender: Sender<RuntimePluginDirective>,
    ) -> Result<()> {
        let mut routes = self.directives.write();
        if !routes.active_plugins.contains(plugin_id) {
            return Err(crate::op_error!(
                "plugin `{plugin_id}` was deactivated during creation"
            ));
        }
        routes
            .senders
            .entry(plugin_id.to_string())
            .or_default()
            .push(sender);
        Ok(())
    }

    fn resolve_capability(
        &self,
        plugin_id: &str,
        kind: AbilityKind,
        type_id: &str,
    ) -> Result<(RuntimePluginInfo, RuntimeCapabilityDescriptor)> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Err(crate::op_error!("plugin_id is empty"));
        }
        let type_id = type_id.trim();
        if type_id.is_empty() {
            return Err(crate::op_error!("type_id is empty"));
        }

        let plugins = self.plugins.read();
        let Some(record) = plugins.get(plugin_id) else {
            return Err(crate::op_error!("plugin `{plugin_id}` is not installed"));
        };
        let Some(capability) = record
            .capabilities
            .iter()
            .find(|cap| cap.kind == kind && cap.type_id == type_id)
            .cloned()
        else {
            return Err(crate::op_error!(
                "plugin `{plugin_id}` has no capability kind=`{:?}` type_id=`{type_id}`",
                kind
            ));
        };
        Ok((record.info.clone(), capability))
    }

    pub(crate) fn load_component_cached(&self, component_path: &Path) -> Result<Component> {
        let cache_key = component_path.to_path_buf();

        {
            let cache = self.component_cache.read();
            if let Some(component) = cache.get(&cache_key) {
                return Ok(component.clone());
            }
        }

        let component = Component::from_file(&self.engine, &cache_key).map_err(|error| {
            crate::op_error!(
                "failed to load component from `{}`: {error:#}",
                cache_key.display()
            )
        })?;

        let mut cache = self.component_cache.write();
        if let Some(component) = cache.get(&cache_key) {
            return Ok(component.clone());
        }
        cache.insert(cache_key, component.clone());
        Ok(component)
    }

    pub(crate) fn remove_cached_components_for_plugin(
        &self,
        plugin: &RuntimePluginInfo,
        capabilities: &[RuntimeCapabilityDescriptor],
    ) {
        let mut cache = self.component_cache.write();
        for capability in capabilities {
            let component_path = plugin.root_dir.join(&capability.component_rel_path);
            cache.remove(&component_path);
        }
    }

    pub(crate) fn clear_component_cache(&self) {
        self.component_cache.write().clear();
    }

    fn instantiate_lyrics_component(
        &self,
        plugin_root: &Path,
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<LyricsStoreData>, LyricsPluginBinding>> {
        let mut store = Store::new(&self.engine, self.new_lyrics_store_data(plugin_root));
        let instance = LyricsPluginBinding::instantiate(&mut store, component, &self.lyrics_linker)
            .map_err(|error| {
                crate::op_error!("failed to instantiate lyrics component: {error:#}")
            })?;
        let on_enable = instance
            .stellatune_plugin_lifecycle()
            .call_on_enable(&mut store)
            .map_err(|error| crate::op_error!("lifecycle.on-enable call failed: {error:#}"))?;
        on_enable
            .map_err(|error| crate::op_error!("lifecycle.on-enable plugin error: {error:?}"))?;
        Ok(PluginCell::new(store, instance, rx))
    }

    fn instantiate_decoder_component(
        &self,
        plugin_root: &Path,
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<DecoderStoreData>, DecoderPluginBinding>> {
        let mut store = Store::new(&self.engine, self.new_decoder_store_data(plugin_root));
        let instance =
            DecoderPluginBinding::instantiate(&mut store, component, &self.decoder_linker)?;
        let on_enable = instance
            .stellatune_plugin_lifecycle()
            .call_on_enable(&mut store)?;
        on_enable
            .map_err(|error| crate::op_error!("lifecycle.on-enable plugin error: {error:?}"))?;
        Ok(PluginCell::new(store, instance, rx))
    }

    fn instantiate_source_component(
        &self,
        plugin_root: &Path,
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<SourceStoreData>, SourcePluginBinding>> {
        let mut store = Store::new(&self.engine, self.new_source_store_data(plugin_root));
        let instance = SourcePluginBinding::instantiate(&mut store, component, &self.source_linker)
            .map_err(|error| {
                crate::op_error!("failed to instantiate source component: {error:#}")
            })?;
        let on_enable = instance
            .stellatune_plugin_lifecycle()
            .call_on_enable(&mut store)
            .map_err(|error| crate::op_error!("lifecycle.on-enable call failed: {error:#}"))?;
        on_enable
            .map_err(|error| crate::op_error!("lifecycle.on-enable plugin error: {error:?}"))?;
        Ok(PluginCell::new(store, instance, rx))
    }

    fn instantiate_output_sink_component(
        &self,
        plugin_root: &Path,
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<OutputSinkStoreData>, OutputSinkPluginBinding>> {
        let mut store = Store::new(&self.engine, self.new_output_sink_store_data(plugin_root));
        let instance =
            OutputSinkPluginBinding::instantiate(&mut store, component, &self.output_sink_linker)
                .map_err(|error| {
                crate::op_error!("failed to instantiate output-sink component: {error:#}")
            })?;
        let on_enable = instance
            .stellatune_plugin_lifecycle()
            .call_on_enable(&mut store)
            .map_err(|error| crate::op_error!("lifecycle.on-enable call failed: {error:#}"))?;
        on_enable
            .map_err(|error| crate::op_error!("lifecycle.on-enable plugin error: {error:?}"))?;
        Ok(PluginCell::new(store, instance, rx))
    }

    fn instantiate_dsp_component(
        &self,
        plugin_root: &Path,
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<DspStoreData>, DspPluginBinding>> {
        let mut store = Store::new(&self.engine, self.new_dsp_store_data(plugin_root));
        let instance = DspPluginBinding::instantiate(&mut store, component, &self.dsp_linker)
            .map_err(|error| crate::op_error!("failed to instantiate dsp component: {error:#}"))?;
        let on_enable = instance
            .stellatune_plugin_lifecycle()
            .call_on_enable(&mut store)
            .map_err(|error| crate::op_error!("lifecycle.on-enable call failed: {error:#}"))?;
        on_enable
            .map_err(|error| crate::op_error!("lifecycle.on-enable plugin error: {error:?}"))?;
        Ok(PluginCell::new(store, instance, rx))
    }
}

fn map_disable_reason_decoder(reason: PluginDisableReason) -> decoder_lifecycle::DisableReason {
    match reason {
        PluginDisableReason::HostDisable => decoder_lifecycle::DisableReason::HostDisable,
        PluginDisableReason::Unload => decoder_lifecycle::DisableReason::Unload,
        PluginDisableReason::Shutdown => decoder_lifecycle::DisableReason::Shutdown,
        PluginDisableReason::Reload => decoder_lifecycle::DisableReason::Reload,
    }
}

fn map_disable_reason_lyrics(reason: PluginDisableReason) -> lyrics_lifecycle::DisableReason {
    match reason {
        PluginDisableReason::HostDisable => lyrics_lifecycle::DisableReason::HostDisable,
        PluginDisableReason::Unload => lyrics_lifecycle::DisableReason::Unload,
        PluginDisableReason::Shutdown => lyrics_lifecycle::DisableReason::Shutdown,
        PluginDisableReason::Reload => lyrics_lifecycle::DisableReason::Reload,
    }
}

fn map_disable_reason_source(reason: PluginDisableReason) -> source_lifecycle::DisableReason {
    match reason {
        PluginDisableReason::HostDisable => source_lifecycle::DisableReason::HostDisable,
        PluginDisableReason::Unload => source_lifecycle::DisableReason::Unload,
        PluginDisableReason::Shutdown => source_lifecycle::DisableReason::Shutdown,
        PluginDisableReason::Reload => source_lifecycle::DisableReason::Reload,
    }
}

fn map_disable_reason_output_sink(
    reason: PluginDisableReason,
) -> output_sink_lifecycle::DisableReason {
    match reason {
        PluginDisableReason::HostDisable => output_sink_lifecycle::DisableReason::HostDisable,
        PluginDisableReason::Unload => output_sink_lifecycle::DisableReason::Unload,
        PluginDisableReason::Shutdown => output_sink_lifecycle::DisableReason::Shutdown,
        PluginDisableReason::Reload => output_sink_lifecycle::DisableReason::Reload,
    }
}

fn map_disable_reason_dsp(reason: PluginDisableReason) -> dsp_lifecycle::DisableReason {
    match reason {
        PluginDisableReason::HostDisable => dsp_lifecycle::DisableReason::HostDisable,
        PluginDisableReason::Unload => dsp_lifecycle::DisableReason::Unload,
        PluginDisableReason::Shutdown => dsp_lifecycle::DisableReason::Shutdown,
        PluginDisableReason::Reload => dsp_lifecycle::DisableReason::Reload,
    }
}

mod controller;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorldKind {
    Decoder,
    Source,
    Lyrics,
    OutputSink,
    Dsp,
    Unknown,
}

fn classify_world(world: &str) -> WorldKind {
    let normalized = normalize_world_name(world);
    match normalized {
        wasm_host::WORLD_DECODER_PLUGIN => WorldKind::Decoder,
        wasm_host::WORLD_SOURCE_PLUGIN => WorldKind::Source,
        wasm_host::WORLD_LYRICS_PLUGIN => WorldKind::Lyrics,
        wasm_host::WORLD_OUTPUT_SINK_PLUGIN => WorldKind::OutputSink,
        wasm_host::WORLD_DSP_PLUGIN => WorldKind::Dsp,
        _ => WorldKind::Unknown,
    }
}

fn normalize_world_name(world: &str) -> &str {
    let world = world.trim();
    let world = world.rsplit('/').next().unwrap_or(world);
    world.split('@').next().unwrap_or(world).trim()
}

fn create_store_wasi_state() -> (WasiCtx, ResourceTable) {
    (
        WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build(),
        ResourceTable::new(),
    )
}
