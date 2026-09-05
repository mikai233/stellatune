use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringbuf::traits::{Consumer as _, Observer as _, Producer as _, Split as _};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use crate::output_runtime::{
    AudioBackend, OutputHandle, OutputSpec, SampleConsumer, default_output_spec, list_host_devices,
    output_spec_for_device,
};
use stellatune_audio_core::{
    error::SinkError,
    format::{AudioBlock, ChannelLayout, PcmFormat},
    sink::{SinkClockSnapshot, SinkStage, SinkWriteResult, SinkWriteState},
};

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
    pub channel_layout: ChannelLayout,
}

impl OutputDeviceSpec {
    pub fn channel_count(self) -> u16 {
        self.channel_layout.channel_count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDeviceDescriptor {
    pub backend: OutputBackend,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceSinkMetricsSnapshot {
    pub ring_capacity_ms: u64,
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
    ring_capacity_ms: AtomicU64,
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
            ring_capacity_ms: self.ring_capacity_ms.load(Ordering::Relaxed),
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
        channel_layout: spec.channel_layout,
    })
}

pub fn output_spec_for_route(
    backend: OutputBackend,
    device_id: Option<&str>,
) -> Result<OutputDeviceSpec, String> {
    let spec = output_spec_for_device(
        backend.to_audio_backend(),
        normalize_device_id_ref(device_id).map(str::to_string),
    )
    .map_err(|e| format!("{e}"))?;
    Ok(OutputDeviceSpec {
        sample_rate: spec.sample_rate.max(1),
        channel_layout: spec.channel_layout,
    })
}

pub struct DeviceSinkStage {
    buffering: stellatune_audio_core::buffering::BufferingConfig,
    control: DeviceSinkControl,
    producer: Option<HeapProd<f32>>,
    output_handle: Option<OutputHandle>,
    callback_error: Arc<Mutex<Option<String>>>,
    prepared_spec: Option<PcmFormat>,
    clock_base_samples: u64,
    epoch: u64,
}

impl DeviceSinkStage {
    pub fn new() -> Self {
        Self::with_control(DeviceSinkControl::default())
    }

    pub fn with_control(control: DeviceSinkControl) -> Self {
        Self {
            buffering: Default::default(),
            control,
            producer: None,
            output_handle: None,
            callback_error: Arc::new(Mutex::new(None)),
            prepared_spec: None,
            clock_base_samples: 0,
            epoch: 0,
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

    fn rebuild_from_control(&mut self) -> Result<(), SinkError> {
        let Some(spec) = self.prepared_spec else {
            return Err(SinkError::Failed {
                message: "device sink is not open".to_owned(),
            });
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
        spec: PcmFormat,
    ) -> Result<(OutputBackend, Option<String>), SinkError> {
        let (backend, desired_device_id) = self.control.desired_route();
        let channels = usize::from(spec.channel_layout.channel_count());
        let capacity_frames =
            stellatune_audio_core::buffering::frames_for_ms(spec, self.buffering.device_ms);
        let capacity_samples = capacity_frames.saturating_mul(channels);
        let rb = HeapRb::<f32>::new(capacity_samples);
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
                channel_layout: spec.channel_layout,
            },
            on_error,
        )
        .map_err(|e| SinkError::Failed {
            message: format!("open output handle failed: {e}"),
        })?;

        self.producer = Some(producer);
        self.output_handle = Some(output_handle);
        self.control.inner.metrics.ring_capacity_ms.store(
            (capacity_frames as u64 * 1000).div_ceil(u64::from(spec.sample_rate)),
            Ordering::Relaxed,
        );
        Ok((backend, desired_device_id))
    }
}

impl Default for DeviceSinkStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SinkStage for DeviceSinkStage {
    fn configure_buffering(&mut self, config: stellatune_audio_core::buffering::BufferingConfig) {
        self.buffering = config;
    }
    fn open(&mut self, format: PcmFormat) -> Result<(), SinkError> {
        self.close();
        self.prepared_spec = Some(format.validate().map_err(|message| SinkError::Failed {
            message: message.to_owned(),
        })?);
        self.clock_base_samples = self.control.metrics_snapshot().callback_provided_samples;
        self.rebuild_from_control()
    }

    fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
        if let Some(error) = self.take_callback_error() {
            return Err(SinkError::Failed { message: error });
        }
        if self.control.needs_reconfigure() {
            self.rebuild_from_control()?;
        }
        let Some(producer) = self.producer.as_mut() else {
            return Err(SinkError::Failed {
                message: "device sink is not open".to_owned(),
            });
        };
        let channels = usize::from(block.format.channel_layout.channel_count());
        let consumed_samples = push_complete_frames(producer, &block.samples, channels);
        self.control.note_written_samples(consumed_samples);
        Ok(SinkWriteResult {
            consumed_frames: consumed_samples / channels,
            state: if consumed_samples < block.samples.len() {
                SinkWriteState::WouldBlock
            } else {
                SinkWriteState::Ready
            },
        })
    }

    fn pause(&mut self) -> Result<(), SinkError> {
        self.output_handle
            .as_ref()
            .ok_or_else(|| SinkError::Failed {
                message: "device sink is not open".to_owned(),
            })?
            .pause()
            .map_err(|error| SinkError::Failed {
                message: error.to_string(),
            })
    }

    fn resume(&mut self) -> Result<(), SinkError> {
        self.output_handle
            .as_ref()
            .ok_or_else(|| SinkError::Failed {
                message: "device sink is not open".to_owned(),
            })?
            .resume()
            .map_err(|error| SinkError::Failed {
                message: error.to_string(),
            })
    }

    fn drain(&mut self) -> Result<(), SinkError> {
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
            return Err(SinkError::Failed { message: error });
        }
        Ok(())
    }

    fn discard(&mut self) -> Result<(), SinkError> {
        let format = self.prepared_spec.ok_or_else(|| SinkError::Failed {
            message: "device sink is not open".to_owned(),
        })?;
        self.epoch = self.epoch.wrapping_add(1);
        self.close();
        self.open(format)
    }

    fn clock_snapshot(&self) -> SinkClockSnapshot {
        let metrics = self.control.metrics_snapshot();
        let channels = u64::from(
            self.prepared_spec
                .map(|format| format.channel_layout.channel_count())
                .unwrap_or(1)
                .max(1),
        );
        let consumed_samples = metrics
            .callback_provided_samples
            .saturating_sub(self.clock_base_samples);
        SinkClockSnapshot {
            consumed_frames: consumed_samples / channels,
            buffered_frames: self
                .producer
                .as_ref()
                .map(|producer| producer.occupied_len() as u64 / channels)
                .unwrap_or(0),
            epoch: self.epoch,
        }
    }

    fn close(&mut self) {
        self.producer = None;
        self.output_handle = None;
        self.control
            .inner
            .metrics
            .ring_capacity_ms
            .store(0, Ordering::Relaxed);
        self.prepared_spec = None;
        self.clear_callback_error();
    }
}

fn push_complete_frames(producer: &mut HeapProd<f32>, samples: &[f32], channels: usize) -> usize {
    let channels = channels.max(1);
    let writable_samples = producer.vacant_len().min(samples.len());
    let complete_frame_samples = writable_samples - (writable_samples % channels);
    if complete_frame_samples == 0 {
        return 0;
    }
    producer.push_slice(&samples[..complete_frame_samples])
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

#[cfg(test)]
mod tests {
    use ringbuf::HeapRb;
    use ringbuf::traits::{Consumer as _, Split as _};

    use super::push_complete_frames;

    #[test]
    fn partial_ring_write_never_splits_an_audio_frame() {
        let ring = HeapRb::<f32>::new(5);
        let (mut producer, mut consumer) = ring.split();
        let input = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];

        let written = push_complete_frames(&mut producer, &input, 2);

        assert_eq!(written, 4);
        let mut output = [0.0; 5];
        assert_eq!(consumer.pop_slice(&mut output), 4);
        assert_eq!(&output[..4], &input[..4]);
    }
}
