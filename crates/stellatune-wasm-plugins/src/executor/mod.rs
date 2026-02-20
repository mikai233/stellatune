use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::error::Result;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store};

use stellatune_wasm_host_bindings as wasm_host;
use stellatune_wasm_host_bindings::generated::decoder_plugin::DecoderPlugin as DecoderPluginBinding;
use stellatune_wasm_host_bindings::generated::decoder_plugin::exports::stellatune::plugin::lifecycle as decoder_lifecycle;
use stellatune_wasm_host_bindings::generated::dsp_plugin::DspPlugin as DspPluginBinding;
use stellatune_wasm_host_bindings::generated::dsp_plugin::exports::stellatune::plugin::lifecycle as dsp_lifecycle;
use stellatune_wasm_host_bindings::generated::lyrics_plugin::LyricsPlugin as LyricsPluginBinding;
use stellatune_wasm_host_bindings::generated::lyrics_plugin::exports::stellatune::plugin::lifecycle as lyrics_lifecycle;
use stellatune_wasm_host_bindings::generated::output_sink_plugin::OutputSinkPlugin as OutputSinkPluginBinding;
use stellatune_wasm_host_bindings::generated::output_sink_plugin::exports::stellatune::plugin::lifecycle as output_sink_lifecycle;
use stellatune_wasm_host_bindings::generated::source_plugin::SourcePlugin as SourcePluginBinding;
use stellatune_wasm_host_bindings::generated::source_plugin::exports::stellatune::plugin::lifecycle as source_lifecycle;

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
    directives: Mutex<DirectiveRegistry>,
    plugins: Mutex<BTreeMap<String, ActivePluginRecord>>,
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
        let engine = Engine::new(&config)?;
        Ok(Self {
            engine,
            http_client,
            stream_service,
            sidecar_host,
            directives: Mutex::new(DirectiveRegistry::default()),
            plugins: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn shared(
        http_client: Arc<dyn HttpClientHost>,
        stream_service: Arc<dyn HostStreamService>,
    ) -> Result<Arc<Self>> {
        Ok(Arc::new(Self::new(http_client, stream_service)?))
    }

    fn new_decoder_store_data(&self) -> DecoderStoreData {
        DecoderStoreData {
            stream_service: self.stream_service.clone(),
            next_rep: 1,
            streams: BTreeMap::new(),
            sidecar: SidecarState::new(self.sidecar_host.clone()),
        }
    }

    fn new_source_store_data(&self) -> SourceStoreData {
        SourceStoreData {
            stream_service: self.stream_service.clone(),
            next_rep: 1,
            streams: BTreeMap::new(),
            sidecar: SidecarState::new(self.sidecar_host.clone()),
        }
    }

    fn new_lyrics_store_data(&self) -> LyricsStoreData {
        LyricsStoreData {
            http_client: self.http_client.clone(),
            sidecar: SidecarState::new(self.sidecar_host.clone()),
        }
    }

    fn new_output_sink_store_data(&self) -> OutputSinkStoreData {
        OutputSinkStoreData {
            sidecar: SidecarState::new(self.sidecar_host.clone()),
        }
    }

    fn new_dsp_store_data(&self) -> DspStoreData {
        DspStoreData {
            sidecar: SidecarState::new(self.sidecar_host.clone()),
        }
    }

    fn ensure_plugin_active(&self, plugin_id: &str) -> Result<()> {
        let routes = self
            .directives
            .lock()
            .expect("executor directives lock poisoned");
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
        let mut routes = self
            .directives
            .lock()
            .expect("executor directives lock poisoned");
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

        let plugins = self.plugins.lock().expect("executor plugin lock poisoned");
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

    fn instantiate_lyrics_component(
        &self,
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<LyricsStoreData>, LyricsPluginBinding>> {
        let mut linker: Linker<LyricsStoreData> = Linker::new(&self.engine);
        LyricsPluginBinding::add_to_linker::<_, HasSelf<LyricsStoreData>>(&mut linker, |state| {
            state
        })?;

        let mut store = Store::new(&self.engine, self.new_lyrics_store_data());
        let instance =
            LyricsPluginBinding::instantiate(&mut store, component, &linker).map_err(|error| {
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
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<DecoderStoreData>, DecoderPluginBinding>> {
        let mut linker: Linker<DecoderStoreData> = Linker::new(&self.engine);
        DecoderPluginBinding::add_to_linker::<_, HasSelf<DecoderStoreData>>(
            &mut linker,
            |state| state,
        )?;
        let mut store = Store::new(&self.engine, self.new_decoder_store_data());
        let instance = DecoderPluginBinding::instantiate(&mut store, component, &linker)?;
        let on_enable = instance
            .stellatune_plugin_lifecycle()
            .call_on_enable(&mut store)?;
        on_enable
            .map_err(|error| crate::op_error!("lifecycle.on-enable plugin error: {error:?}"))?;
        Ok(PluginCell::new(store, instance, rx))
    }

    fn instantiate_source_component(
        &self,
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<SourceStoreData>, SourcePluginBinding>> {
        let mut linker: Linker<SourceStoreData> = Linker::new(&self.engine);
        SourcePluginBinding::add_to_linker::<_, HasSelf<SourceStoreData>>(&mut linker, |state| {
            state
        })?;
        let mut store = Store::new(&self.engine, self.new_source_store_data());
        let instance =
            SourcePluginBinding::instantiate(&mut store, component, &linker).map_err(|error| {
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
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<OutputSinkStoreData>, OutputSinkPluginBinding>> {
        let mut linker: Linker<OutputSinkStoreData> = Linker::new(&self.engine);
        OutputSinkPluginBinding::add_to_linker::<_, HasSelf<OutputSinkStoreData>>(
            &mut linker,
            |state| state,
        )?;
        let mut store = Store::new(&self.engine, self.new_output_sink_store_data());
        let instance = OutputSinkPluginBinding::instantiate(&mut store, component, &linker)
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
        component: &Component,
        rx: Receiver<RuntimePluginDirective>,
    ) -> Result<PluginCell<Store<DspStoreData>, DspPluginBinding>> {
        let mut linker: Linker<DspStoreData> = Linker::new(&self.engine);
        DspPluginBinding::add_to_linker::<_, HasSelf<DspStoreData>>(&mut linker, |state| state)?;
        let mut store = Store::new(&self.engine, self.new_dsp_store_data());
        let instance = DspPluginBinding::instantiate(&mut store, component, &linker)
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
