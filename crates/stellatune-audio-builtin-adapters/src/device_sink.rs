use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringbuf::traits::{Consumer as _, Observer as _, Producer as _, Split as _};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use crate::output_runtime::{
    AudioBackend, OutputHandle, OutputSpec, SampleConsumer, default_output_spec, list_host_devices,
    output_spec_for_device,
};
use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;

const RING_BUFFER_CAPACITY_MS: usize = 40;
const WRITE_BACKPRESSURE_TIMEOUT_MS: u64 = 30;
const WRITE_BACKPRESSURE_SLEEP_MS: u64 = 1;
const FLUSH_TIMEOUT_MS: u64 = 350;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputBackend {
    Shared,
    WasapiExclusive,
}

impl OutputBackend {
    fn to_audio_backend(self) -> AudioBackend {
        match self {
            Self::Shared => AudioBackend::Shared,
            Self::WasapiExclusive => AudioBackend::WasapiExclusive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputDeviceSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDeviceDescriptor {
    pub backend: OutputBackend,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceSinkMetricsSnapshot {
    pub written_samples: u64,
    pub dropped_samples: u64,
    pub callback_requested_samples: u64,
    pub callback_provided_samples: u64,
    pub underrun_callbacks: u64,
    pub callback_errors: u64,
    pub reconfigure_attempts: u64,
    pub reconfigure_successes: u64,
    pub reconfigure_failures: u64,
}

#[derive(Debug, Default)]
struct DeviceSinkMetrics {
    written_samples: AtomicU64,
    dropped_samples: AtomicU64,
    callback_requested_samples: AtomicU64,
    callback_provided_samples: AtomicU64,
    underrun_callbacks: AtomicU64,
    callback_errors: AtomicU64,
    reconfigure_attempts: AtomicU64,
    reconfigure_successes: AtomicU64,
    reconfigure_failures: AtomicU64,
}

impl DeviceSinkMetrics {
    fn snapshot(&self) -> DeviceSinkMetricsSnapshot {
        DeviceSinkMetricsSnapshot {
            written_samples: self.written_samples.load(Ordering::Relaxed),
            dropped_samples: self.dropped_samples.load(Ordering::Relaxed),
            callback_requested_samples: self.callback_requested_samples.load(Ordering::Relaxed),
            callback_provided_samples: self.callback_provided_samples.load(Ordering::Relaxed),
            underrun_callbacks: self.underrun_callbacks.load(Ordering::Relaxed),
            callback_errors: self.callback_errors.load(Ordering::Relaxed),
            reconfigure_attempts: self.reconfigure_attempts.load(Ordering::Relaxed),
            reconfigure_successes: self.reconfigure_successes.load(Ordering::Relaxed),
            reconfigure_failures: self.reconfigure_failures.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
struct DeviceSinkControlInner {
    desired_backend: Mutex<OutputBackend>,
    desired_device_id: Mutex<Option<String>>,
    desired_revision: AtomicU64,
    applied_backend: Mutex<OutputBackend>,
    applied_device_id: Mutex<Option<String>>,
    applied_revision: AtomicU64,
    metrics: DeviceSinkMetrics,
}

impl Default for DeviceSinkControlInner {
    fn default() -> Self {
        Self {
            desired_backend: Mutex::new(OutputBackend::Shared),
            desired_device_id: Mutex::new(None),
            desired_revision: AtomicU64::new(0),
            applied_backend: Mutex::new(OutputBackend::Shared),
            applied_device_id: Mutex::new(None),
            applied_revision: AtomicU64::new(0),
            metrics: DeviceSinkMetrics::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeviceSinkControl {
    inner: Arc<DeviceSinkControlInner>,
}

impl DeviceSinkControl {
    pub fn set_route(&self, backend: OutputBackend, device_id: Option<String>) {
        let normalized_device_id = normalize_device_id_owned(device_id);
        let mut changed = false;
        if let Ok(mut desired_backend) = self.inner.desired_backend.lock()
            && *desired_backend != backend
        {
            *desired_backend = backend;
            changed = true;
        }
        if let Ok(mut desired_device_id) = self.inner.desired_device_id.lock()
            && *desired_device_id != normalized_device_id
        {
            *desired_device_id = normalized_device_id;
            changed = true;
        }
        if changed {
            self.inner.desired_revision.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn set_device_id(&self, device_id: Option<String>) {
        self.set_route(OutputBackend::Shared, device_id);
    }

    pub fn desired_route(&self) -> (OutputBackend, Option<String>) {
        let backend = self
            .inner
            .desired_backend
            .lock()
            .map(|value| *value)
            .unwrap_or(OutputBackend::Shared);
        let device_id = self
            .inner
            .desired_device_id
            .lock()
            .ok()
            .and_then(|value| value.clone());
        (backend, device_id)
    }

    pub fn desired_backend(&self) -> OutputBackend {
        self.desired_route().0
    }

    pub fn desired_device_id(&self) -> Option<String> {
        self.desired_route().1
    }

    pub fn metrics_snapshot(&self) -> DeviceSinkMetricsSnapshot {
        self.inner.metrics.snapshot()
    }

    fn desired_revision(&self) -> u64 {
        self.inner.desired_revision.load(Ordering::Relaxed)
    }

    fn applied_revision(&self) -> u64 {
        self.inner.applied_revision.load(Ordering::Relaxed)
    }

    fn needs_reconfigure(&self) -> bool {
        self.desired_revision() != self.applied_revision()
    }

    fn mark_applied(&self, backend: OutputBackend, applied_device_id: Option<String>) {
        if let Ok(mut applied_backend) = self.inner.applied_backend.lock() {
            *applied_backend = backend;
        }
        if let Ok(mut applied_device_id_slot) = self.inner.applied_device_id.lock() {
            *applied_device_id_slot = normalize_device_id_owned(applied_device_id);
        }
        self.inner
            .applied_revision
            .store(self.desired_revision(), Ordering::Relaxed);
    }

    fn note_written_samples(&self, samples: usize) {
        self.inner
            .metrics
            .written_samples
            .fetch_add(samples as u64, Ordering::Relaxed);
    }

    fn note_dropped_samples(&self, samples: usize) {
        self.inner
            .metrics
            .dropped_samples
            .fetch_add(samples as u64, Ordering::Relaxed);
    }

    fn note_callback(&self, requested_samples: usize, provided_samples: usize) {
        self.inner
            .metrics
            .callback_requested_samples
            .fetch_add(requested_samples as u64, Ordering::Relaxed);
        self.inner
            .metrics
            .callback_provided_samples
            .fetch_add(provided_samples as u64, Ordering::Relaxed);
        if provided_samples < requested_samples {
            self.inner
                .metrics
                .underrun_callbacks
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    fn note_callback_error(&self) {
        self.inner
            .metrics
            .callback_errors
            .fetch_add(1, Ordering::Relaxed);
    }

    fn note_reconfigure_attempt(&self) {
        self.inner
            .metrics
            .reconfigure_attempts
            .fetch_add(1, Ordering::Relaxed);
    }

    fn note_reconfigure_success(&self) {
        self.inner
            .metrics
            .reconfigure_successes
            .fetch_add(1, Ordering::Relaxed);
    }

    fn note_reconfigure_failure(&self) {
        self.inner
            .metrics
            .reconfigure_failures
            .fetch_add(1, Ordering::Relaxed);
    }
}

pub fn list_output_devices() -> Result<Vec<OutputDeviceDescriptor>, String> {
    let devices = list_host_devices(None)
        .into_iter()
        .map(|device| OutputDeviceDescriptor {
            backend: map_audio_backend(device.backend),
            id: device.id,
            name: device.name,
        })
        .collect::<Vec<_>>();
    Ok(devices)
}

pub fn default_output_spec_for_backend(backend: OutputBackend) -> Result<OutputDeviceSpec, String> {
    let backend = backend.to_audio_backend();
    let spec = match backend {
        AudioBackend::Shared => default_output_spec(),
        AudioBackend::WasapiExclusive => output_spec_for_device(backend, None),
    }
    .map_err(|e| format!("{e}"))?;
    Ok(OutputDeviceSpec {
        sample_rate: spec.sample_rate.max(1),
        channels: spec.channels.max(1),
    })
}

pub fn output_spec_for_route(
    backend: OutputBackend,
    device_id: Option<&str>,
) -> Result<OutputDeviceSpec, String> {
    let spec = output_spec_for_device(
        backend.to_audio_backend(),
        normalize_device_id_ref(device_id.as_deref()).map(str::to_string),
    )
    .map_err(|e| format!("{e}"))?;
    Ok(OutputDeviceSpec {
        sample_rate: spec.sample_rate.max(1),
        channels: spec.channels.max(1),
    })
}

pub struct DeviceSinkStage {
    control: DeviceSinkControl,
    producer: Option<HeapProd<f32>>,
    output_handle: Option<OutputHandle>,
    callback_error: Arc<Mutex<Option<String>>>,
    prepared_spec: Option<StreamSpec>,
}

impl DeviceSinkStage {
    pub fn new() -> Self {
        Self::with_control(DeviceSinkControl::default())
    }

    pub fn with_control(control: DeviceSinkControl) -> Self {
        Self {
            control,
            producer: None,
            output_handle: None,
            callback_error: Arc::new(Mutex::new(None)),
            prepared_spec: None,
        }
    }

    pub fn control(&self) -> DeviceSinkControl {
        self.control.clone()
    }

    fn clear_callback_error(&self) {
        if let Ok(mut slot) = self.callback_error.lock() {
            *slot = None;
        }
    }

    fn take_callback_error(&self) -> Option<String> {
        self.callback_error
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
    }

    fn rebuild_from_control(&mut self) -> Result<(), PipelineError> {
        let Some(spec) = self.prepared_spec else {
            return Err(PipelineError::StageFailure(
                "device sink is not prepared".to_string(),
            ));
        };

        self.control.note_reconfigure_attempt();
        self.producer = None;
        self.output_handle = None;
        self.clear_callback_error();

        match self.open_stream(spec) {
            Ok((backend, applied_device_id)) => {
                self.control.mark_applied(backend, applied_device_id);
                self.control.note_reconfigure_success();
                Ok(())
            },
            Err(error) => {
                self.control.note_reconfigure_failure();
                Err(error)
            },
        }
    }

    fn open_stream(
        &mut self,
        spec: StreamSpec,
    ) -> Result<(OutputBackend, Option<String>), PipelineError> {
        let (backend, desired_device_id) = self.control.desired_route();
        let capacity_samples =
            ((spec.sample_rate as usize * spec.channels as usize * RING_BUFFER_CAPACITY_MS) / 1000)
                .max(spec.channels as usize * 1024);
        let rb = HeapRb::<f32>::new(capacity_samples.max(1024));
        let (producer, consumer) = rb.split();

        let callback_error = Arc::clone(&self.callback_error);
        let metrics = self.control.clone();
        let on_error = move |error: String| {
            metrics.note_callback_error();
            if let Ok(mut slot) = callback_error.lock() {
                *slot = Some(format!("output stream error: {error}"));
            }
        };

        let output_handle = OutputHandle::start(
            backend.to_audio_backend(),
            desired_device_id.clone(),
            RingBufferConsumer {
                consumer,
                metrics: self.control.clone(),
            },
            OutputSpec {
                sample_rate: spec.sample_rate.max(1),
                channels: spec.channels.max(1),
            },
            on_error,
        )
        .map_err(|e| PipelineError::StageFailure(format!("open output handle failed: {e}")))?;

        self.producer = Some(producer);
        self.output_handle = Some(output_handle);
        Ok((backend, desired_device_id))
    }
}

impl Default for DeviceSinkStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SinkStage for DeviceSinkStage {
    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.stop(_ctx);
        self.prepared_spec = Some(spec);
        self.rebuild_from_control()
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        let stream_error = self.take_callback_error();
        if stream_error.is_some() || self.control.needs_reconfigure() {
            return self.rebuild_from_control().map_err(|error| {
                if let Some(stream_error) = stream_error {
                    PipelineError::StageFailure(format!(
                        "{stream_error}; sink reconfigure failed: {error}"
                    ))
                } else {
                    error
                }
            });
        }
        Ok(())
    }

    fn write(&mut self, block: &AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if self.take_callback_error().is_some() {
            return StageStatus::Fatal;
        }
        let Some(producer) = self.producer.as_mut() else {
            return StageStatus::Fatal;
        };

        let samples = block.samples.as_slice();
        let mut offset = 0usize;
        let deadline = Instant::now() + Duration::from_millis(WRITE_BACKPRESSURE_TIMEOUT_MS);
        while offset < samples.len() {
            let pushed = producer.push_slice(&samples[offset..]);
            if pushed > 0 {
                offset = offset.saturating_add(pushed);
                continue;
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(WRITE_BACKPRESSURE_SLEEP_MS));
        }

        self.control.note_written_samples(offset);
        if offset < samples.len() {
            self.control.note_dropped_samples(samples.len() - offset);
        }
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        let Some(producer) = self.producer.as_ref() else {
            return Ok(());
        };
        let deadline = Instant::now() + Duration::from_millis(FLUSH_TIMEOUT_MS);
        while producer.occupied_len() > 0 {
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if let Some(error) = self.take_callback_error() {
            return Err(PipelineError::StageFailure(error));
        }
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.producer = None;
        self.output_handle = None;
        self.prepared_spec = None;
        self.clear_callback_error();
    }
}

struct RingBufferConsumer {
    consumer: HeapCons<f32>,
    metrics: DeviceSinkControl,
}

impl SampleConsumer for RingBufferConsumer {
    fn pop_sample(&mut self) -> Option<f32> {
        let mut sample = [0.0f32];
        if self.consumer.pop_slice(&mut sample) == 1 {
            Some(sample[0])
        } else {
            None
        }
    }

    fn on_output(&mut self, requested: usize, provided: usize) {
        self.metrics.note_callback(requested, provided);
    }
}

fn normalize_device_id_owned(device_id: Option<String>) -> Option<String> {
    device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_device_id_ref(device_id: Option<&str>) -> Option<&str> {
    device_id.map(str::trim).filter(|value| !value.is_empty())
}

fn map_audio_backend(backend: AudioBackend) -> OutputBackend {
    match backend {
        AudioBackend::Shared => OutputBackend::Shared,
        AudioBackend::WasapiExclusive => OutputBackend::WasapiExclusive,
    }
}
