use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;

use crate::device_sink::{
    DeviceSinkControl, DeviceSinkMetricsSnapshot, DeviceSinkStage, OutputBackend,
    OutputDeviceDescriptor, OutputDeviceSpec, default_output_spec_for_backend, list_output_devices,
    output_spec_for_route,
};

#[derive(Debug, Clone)]
pub struct WasapiExclusiveSinkControl {
    inner: DeviceSinkControl,
}

impl WasapiExclusiveSinkControl {
    pub fn new() -> Self {
        let inner = DeviceSinkControl::default();
        inner.set_route(OutputBackend::WasapiExclusive, None);
        Self { inner }
    }

    pub fn from_device_sink_control(inner: DeviceSinkControl) -> Self {
        let control = Self { inner };
        let device_id = control.inner.desired_device_id();
        control
            .inner
            .set_route(OutputBackend::WasapiExclusive, device_id);
        control
    }

    pub fn set_device_id(&self, device_id: Option<String>) {
        self.inner
            .set_route(OutputBackend::WasapiExclusive, device_id);
    }

    pub fn clear_device_id(&self) {
        self.set_device_id(None);
    }

    pub fn desired_device_id(&self) -> Option<String> {
        self.inner.desired_device_id()
    }

    pub fn metrics_snapshot(&self) -> DeviceSinkMetricsSnapshot {
        self.inner.metrics_snapshot()
    }

    pub fn as_device_sink_control(&self) -> DeviceSinkControl {
        self.inner.clone()
    }
}

impl Default for WasapiExclusiveSinkControl {
    fn default() -> Self {
        Self::new()
    }
}

pub fn list_wasapi_exclusive_output_devices() -> Result<Vec<OutputDeviceDescriptor>, String> {
    let mut devices = list_output_devices()?
        .into_iter()
        .filter(|item| item.backend == OutputBackend::WasapiExclusive)
        .collect::<Vec<_>>();
    devices.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(devices)
}

pub fn default_wasapi_exclusive_output_spec() -> Result<OutputDeviceSpec, String> {
    default_output_spec_for_backend(OutputBackend::WasapiExclusive)
}

pub fn output_spec_for_wasapi_exclusive_device(
    device_id: Option<&str>,
) -> Result<OutputDeviceSpec, String> {
    output_spec_for_route(OutputBackend::WasapiExclusive, device_id)
}

pub struct WasapiExclusiveSinkStage {
    inner: DeviceSinkStage,
    control: WasapiExclusiveSinkControl,
}

impl WasapiExclusiveSinkStage {
    pub fn new() -> Self {
        Self::with_control(WasapiExclusiveSinkControl::new())
    }

    pub fn with_control(control: WasapiExclusiveSinkControl) -> Self {
        let inner = DeviceSinkStage::with_control(control.as_device_sink_control());
        Self { inner, control }
    }

    pub fn with_device_sink_control(control: DeviceSinkControl) -> Self {
        Self::with_control(WasapiExclusiveSinkControl::from_device_sink_control(
            control,
        ))
    }

    pub fn control(&self) -> WasapiExclusiveSinkControl {
        self.control.clone()
    }
}

impl Default for WasapiExclusiveSinkStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SinkStage for WasapiExclusiveSinkStage {
    fn prepare(
        &mut self,
        spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.inner.prepare(spec, ctx)
    }

    fn sync_runtime_control(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        self.inner.sync_runtime_control(ctx)
    }

    fn write(&mut self, block: &AudioBlock, ctx: &mut PipelineContext) -> StageStatus {
        self.inner.write(block, ctx)
    }

    fn flush(&mut self, ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        self.inner.flush(ctx)
    }

    fn stop(&mut self, ctx: &mut PipelineContext) {
        self.inner.stop(ctx)
    }
}
