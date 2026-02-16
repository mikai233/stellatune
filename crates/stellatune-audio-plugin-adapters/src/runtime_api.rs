pub use stellatune_plugin_api::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_ERR_INVALID_ARG, ST_ERR_IO, ST_LAYOUT_5_1, ST_LAYOUT_7_1,
    ST_LAYOUT_ANY, ST_LAYOUT_MONO, ST_LAYOUT_STEREO, StAudioSpec, StIoVTable,
    StOutputSinkNegotiatedSpec, StSeekWhence, StStatus, StStr,
};
pub use stellatune_plugins::capabilities::decoder::DecoderInstance as PluginDecoderInstance;
pub use stellatune_plugins::capabilities::dsp::DspInstance as PluginDspInstance;
pub use stellatune_plugins::capabilities::output::OutputSinkInstance as PluginOutputSinkInstance;
pub use stellatune_plugins::runtime::handle::SharedPluginRuntimeHandle;
pub use stellatune_plugins::runtime::introspection::CapabilityKind as RuntimeCapabilityKind;
pub use stellatune_plugins::runtime::messages::WorkerControlMessage;
pub use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};
pub use stellatune_plugins::runtime::worker_endpoint::{
    DecoderWorkerController, DspWorkerController, OutputSinkWorkerController,
    SourceCatalogWorkerController,
};

pub fn shared_runtime_service() -> SharedPluginRuntimeHandle {
    stellatune_plugins::runtime::handle::shared_runtime_service()
}
