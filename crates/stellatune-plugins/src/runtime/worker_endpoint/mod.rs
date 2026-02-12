use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::runtime::actor::WorkerControlMessage;
use crate::runtime::handle::PluginRuntimeHandle;
use crate::runtime::instance_registry::InstanceRegistry;
use crate::runtime::update::InstanceUpdateCoordinator;
use crate::runtime::worker_controller::WorkerInstanceController;

mod common;
mod decoder;
mod dsp;
mod lyrics;
mod output;
mod source;

#[derive(Clone)]
pub struct DecoderInstanceFactory {
    runtime: PluginRuntimeHandle,
    plugin_id: String,
    type_id: String,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
}

#[derive(Clone)]
pub struct DspInstanceFactory {
    runtime: PluginRuntimeHandle,
    plugin_id: String,
    type_id: String,
    sample_rate: u32,
    channels: u16,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
}

#[derive(Clone)]
pub struct SourceCatalogInstanceFactory {
    runtime: PluginRuntimeHandle,
    plugin_id: String,
    type_id: String,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
}

#[derive(Clone)]
pub struct LyricsProviderInstanceFactory {
    runtime: PluginRuntimeHandle,
    plugin_id: String,
    type_id: String,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
}

#[derive(Clone)]
pub struct OutputSinkInstanceFactory {
    runtime: PluginRuntimeHandle,
    plugin_id: String,
    type_id: String,
    instances: Arc<InstanceRegistry>,
    updates: Arc<InstanceUpdateCoordinator>,
}

pub struct DecoderWorkerEndpoint {
    pub factory: DecoderInstanceFactory,
    pub control_rx: Receiver<WorkerControlMessage>,
}

pub struct DspWorkerEndpoint {
    pub factory: DspInstanceFactory,
    pub control_rx: Receiver<WorkerControlMessage>,
}

pub struct SourceCatalogWorkerEndpoint {
    pub factory: SourceCatalogInstanceFactory,
    pub control_rx: Receiver<WorkerControlMessage>,
}

pub struct LyricsProviderWorkerEndpoint {
    pub factory: LyricsProviderInstanceFactory,
    pub control_rx: Receiver<WorkerControlMessage>,
}

pub struct OutputSinkWorkerEndpoint {
    pub factory: OutputSinkInstanceFactory,
    pub control_rx: Receiver<WorkerControlMessage>,
}

pub type DecoderWorkerController = WorkerInstanceController<DecoderInstanceFactory>;
pub type DspWorkerController = WorkerInstanceController<DspInstanceFactory>;
pub type SourceCatalogWorkerController = WorkerInstanceController<SourceCatalogInstanceFactory>;
pub type LyricsProviderWorkerController = WorkerInstanceController<LyricsProviderInstanceFactory>;
pub type OutputSinkWorkerController = WorkerInstanceController<OutputSinkInstanceFactory>;

macro_rules! impl_into_controller {
    ($endpoint:ident, $controller:ident) => {
        impl $endpoint {
            pub fn into_controller(
                self,
                initial_config_json: impl Into<String>,
            ) -> ($controller, Receiver<WorkerControlMessage>) {
                let controller = WorkerInstanceController::new(self.factory, initial_config_json);
                (controller, self.control_rx)
            }
        }
    };
}

impl_into_controller!(DecoderWorkerEndpoint, DecoderWorkerController);
impl_into_controller!(DspWorkerEndpoint, DspWorkerController);
impl_into_controller!(SourceCatalogWorkerEndpoint, SourceCatalogWorkerController);
impl_into_controller!(LyricsProviderWorkerEndpoint, LyricsProviderWorkerController);
impl_into_controller!(OutputSinkWorkerEndpoint, OutputSinkWorkerController);
