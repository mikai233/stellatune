use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::runtime::handle::PluginRuntimeHandle;
use crate::runtime::instance_registry::InstanceRegistry;
use crate::runtime::messages::WorkerControlMessage;
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

pub struct WorkerEndpoint<F> {
    pub factory: F,
    pub control_rx: Receiver<WorkerControlMessage>,
}

impl<F> WorkerEndpoint<F> {
    pub fn into_controller(
        self,
        initial_config_json: impl Into<String>,
    ) -> (WorkerInstanceController<F>, Receiver<WorkerControlMessage>)
    where
        F: crate::runtime::worker_controller::WorkerInstanceFactory,
    {
        let controller = WorkerInstanceController::new(self.factory, initial_config_json);
        (controller, self.control_rx)
    }
}

pub type DecoderWorkerEndpoint = WorkerEndpoint<DecoderInstanceFactory>;
pub type DspWorkerEndpoint = WorkerEndpoint<DspInstanceFactory>;
pub type SourceCatalogWorkerEndpoint = WorkerEndpoint<SourceCatalogInstanceFactory>;
pub type LyricsProviderWorkerEndpoint = WorkerEndpoint<LyricsProviderInstanceFactory>;
pub type OutputSinkWorkerEndpoint = WorkerEndpoint<OutputSinkInstanceFactory>;

pub type DecoderWorkerController = WorkerInstanceController<DecoderInstanceFactory>;
pub type DspWorkerController = WorkerInstanceController<DspInstanceFactory>;
pub type SourceCatalogWorkerController = WorkerInstanceController<SourceCatalogInstanceFactory>;
pub type LyricsProviderWorkerController = WorkerInstanceController<LyricsProviderInstanceFactory>;
pub type OutputSinkWorkerController = WorkerInstanceController<OutputSinkInstanceFactory>;
