use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::{Consumer as _, Observer as _, Producer as _, Split as _};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;

const RING_BUFFER_CAPACITY_MS: usize = 40;
const WRITE_BACKPRESSURE_TIMEOUT_MS: u64 = 30;
const WRITE_BACKPRESSURE_SLEEP_MS: u64 = 1;
const FLUSH_TIMEOUT_MS: u64 = 350;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedOutputSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedOutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SharedDeviceSinkMetricsSnapshot {
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
struct SharedDeviceSinkMetrics {
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

impl SharedDeviceSinkMetrics {
    fn snapshot(&self) -> SharedDeviceSinkMetricsSnapshot {
        SharedDeviceSinkMetricsSnapshot {
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
struct SharedDeviceSinkControlInner {
    desired_device_id: Mutex<Option<String>>,
    desired_revision: AtomicU64,
    applied_device_id: Mutex<Option<String>>,
    applied_revision: AtomicU64,
    metrics: SharedDeviceSinkMetrics,
}

impl Default for SharedDeviceSinkControlInner {
    fn default() -> Self {
        Self {
            desired_device_id: Mutex::new(None),
            desired_revision: AtomicU64::new(0),
            applied_device_id: Mutex::new(None),
            applied_revision: AtomicU64::new(0),
            metrics: SharedDeviceSinkMetrics::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharedDeviceSinkControl {
    inner: Arc<SharedDeviceSinkControlInner>,
}

impl SharedDeviceSinkControl {
    pub fn set_device_id(&self, device_id: Option<String>) {
        let normalized = normalize_device_id_owned(device_id);
        let mut changed = false;
        if let Ok(mut desired) = self.inner.desired_device_id.lock()
            && *desired != normalized
        {
            *desired = normalized;
            changed = true;
        }
        if changed {
            self.inner.desired_revision.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn clear_device_id(&self) {
        self.set_device_id(None);
    }

    pub fn desired_device_id(&self) -> Option<String> {
        self.inner
            .desired_device_id
            .lock()
            .ok()
            .and_then(|value| value.clone())
    }

    pub fn applied_device_id(&self) -> Option<String> {
        self.inner
            .applied_device_id
            .lock()
            .ok()
            .and_then(|value| value.clone())
    }

    pub fn metrics_snapshot(&self) -> SharedDeviceSinkMetricsSnapshot {
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

    fn mark_applied(&self, applied_device_id: Option<String>) {
        if let Ok(mut applied) = self.inner.applied_device_id.lock() {
            *applied = normalize_device_id_owned(applied_device_id);
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

pub fn list_shared_output_devices() -> Result<Vec<SharedOutputDevice>, String> {
    let host = cpal::default_host();
    let default_id = host
        .default_output_device()
        .map(|device| cpal_device_id(&device));
    let mut devices = host
        .output_devices()
        .map_err(|e| format!("failed to query output devices: {e}"))?
        .map(|device| {
            let id = cpal_device_id(&device);
            SharedOutputDevice {
                name: cpal_device_label(&device),
                is_default: default_id.as_deref() == Some(id.as_str()),
                id,
            }
        })
        .collect::<Vec<_>>();
    devices.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(devices)
}

pub fn default_shared_output_spec() -> Result<SharedOutputSpec, String> {
    output_spec_for_shared_device(None)
}

pub fn output_spec_for_shared_device(device_id: Option<&str>) -> Result<SharedOutputSpec, String> {
    let host = cpal::default_host();
    let (device, _actual_device_id) = select_output_device(&host, device_id)?;
    let config = device
        .default_output_config()
        .map_err(|e| format!("failed to query default output config: {e}"))?;
    Ok(SharedOutputSpec {
        sample_rate: config.sample_rate().max(1),
        channels: config.channels().max(1),
    })
}

pub struct SharedDeviceSinkStage {
    control: SharedDeviceSinkControl,
    producer: Option<HeapProd<f32>>,
    stream: Option<cpal::Stream>,
    callback_error: Arc<Mutex<Option<String>>>,
    prepared_spec: Option<StreamSpec>,
}

impl SharedDeviceSinkStage {
    pub fn new() -> Self {
        Self::with_control(SharedDeviceSinkControl::default())
    }

    pub fn with_control(control: SharedDeviceSinkControl) -> Self {
        Self {
            control,
            producer: None,
            stream: None,
            callback_error: Arc::new(Mutex::new(None)),
            prepared_spec: None,
        }
    }

    pub fn control(&self) -> SharedDeviceSinkControl {
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
                "shared device sink is not prepared".to_string(),
            ));
        };

        self.control.note_reconfigure_attempt();
        self.producer = None;
        self.stream = None;
        self.clear_callback_error();

        match self.open_stream(spec) {
            Ok(applied_device_id) => {
                self.control.mark_applied(Some(applied_device_id));
                self.control.note_reconfigure_success();
                Ok(())
            },
            Err(error) => {
                self.control.note_reconfigure_failure();
                Err(error)
            },
        }
    }

    fn open_stream(&mut self, spec: StreamSpec) -> Result<String, PipelineError> {
        let host = cpal::default_host();
        let desired_device = self.control.desired_device_id();
        let (device, applied_device_id) = select_output_device(&host, desired_device.as_deref())
            .map_err(PipelineError::StageFailure)?;

        let default_config = device.default_output_config().map_err(|e| {
            PipelineError::StageFailure(format!("failed to query default output config: {e}"))
        })?;

        let default_sample_rate = default_config.sample_rate().max(1);
        let default_channels = default_config.channels().max(1);
        if default_sample_rate != spec.sample_rate || default_channels != spec.channels {
            return Err(PipelineError::StageFailure(format!(
                "output device spec mismatch: decoder pipeline={}Hz/{}ch, device={}Hz/{}ch; enable mixer/resampler to match output device",
                spec.sample_rate, spec.channels, default_sample_rate, default_channels
            )));
        }

        let stream_config = cpal::StreamConfig {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };
        let capacity_samples =
            ((spec.sample_rate as usize * spec.channels as usize * RING_BUFFER_CAPACITY_MS) / 1000)
                .max(spec.channels as usize * 1024);
        let rb = HeapRb::<f32>::new(capacity_samples.max(1024));
        let (producer, consumer) = rb.split();

        let callback_error = Arc::clone(&self.callback_error);
        let error_metrics = self.control.clone();
        let on_error = move |error: cpal::StreamError| {
            error_metrics.note_callback_error();
            if let Ok(mut slot) = callback_error.lock() {
                *slot = Some(format!("output stream error: {error}"));
            }
        };

        let stream = match default_config.sample_format() {
            cpal::SampleFormat::F32 => {
                let metrics = self.control.clone();
                let mut consumer = consumer;
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [f32], _| {
                        let provided = write_f32_samples(data, &mut consumer);
                        metrics.note_callback(data.len(), provided);
                    },
                    on_error,
                    Some(Duration::from_millis(200)),
                )
            },
            cpal::SampleFormat::I16 => {
                let metrics = self.control.clone();
                let mut consumer = consumer;
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [i16], _| {
                        let provided = write_i16_samples(data, &mut consumer);
                        metrics.note_callback(data.len(), provided);
                    },
                    on_error,
                    Some(Duration::from_millis(200)),
                )
            },
            cpal::SampleFormat::U16 => {
                let metrics = self.control.clone();
                let mut consumer = consumer;
                device.build_output_stream(
                    &stream_config,
                    move |data: &mut [u16], _| {
                        let provided = write_u16_samples(data, &mut consumer);
                        metrics.note_callback(data.len(), provided);
                    },
                    on_error,
                    Some(Duration::from_millis(200)),
                )
            },
            sample_format => {
                return Err(PipelineError::StageFailure(format!(
                    "unsupported output sample format: {sample_format:?}"
                )));
            },
        }
        .map_err(|e| PipelineError::StageFailure(format!("build output stream failed: {e}")))?;

        stream
            .play()
            .map_err(|e| PipelineError::StageFailure(format!("play output stream failed: {e}")))?;
        self.producer = Some(producer);
        self.stream = Some(stream);
        Ok(applied_device_id)
    }
}

impl Default for SharedDeviceSinkStage {
    fn default() -> Self {
        Self::new()
    }
}

impl SinkStage for SharedDeviceSinkStage {
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
        self.stream = None;
        self.prepared_spec = None;
        self.clear_callback_error();
    }
}

fn select_output_device(
    host: &cpal::Host,
    device_id: Option<&str>,
) -> Result<(cpal::Device, String), String> {
    let desired_id = normalize_device_id_ref(device_id);
    if let Some(desired_id) = desired_id {
        let mut devices = host
            .output_devices()
            .map_err(|e| format!("failed to query output devices: {e}"))?;
        if let Some(device) = devices.find(|device| cpal_device_id(device) == desired_id) {
            let resolved_id = cpal_device_id(&device);
            return Ok((device, resolved_id));
        }
    }

    let default_device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())?;
    let default_id = cpal_device_id(&default_device);
    Ok((default_device, default_id))
}

fn cpal_device_label(device: &cpal::Device) -> String {
    match device.description() {
        Ok(desc) => desc
            .extended()
            .first()
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| desc.name().trim())
            .to_string(),
        Err(_) => "Unknown CPAL Device".to_string(),
    }
}

fn cpal_device_id(device: &cpal::Device) -> String {
    device
        .id()
        .ok()
        .map(|id| id.to_string())
        .unwrap_or_else(|| cpal_device_label(device))
}

fn normalize_device_id_owned(device_id: Option<String>) -> Option<String> {
    device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_device_id_ref(device_id: Option<&str>) -> Option<String> {
    device_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn write_f32_samples(data: &mut [f32], consumer: &mut HeapCons<f32>) -> usize {
    let provided = consumer.pop_slice(data);
    if provided < data.len() {
        data[provided..].fill(0.0);
    }
    provided
}

fn write_i16_samples(data: &mut [i16], consumer: &mut HeapCons<f32>) -> usize {
    let mut provided = 0usize;
    let mut scratch = [0.0f32];
    for sample in data {
        if consumer.pop_slice(&mut scratch) == 1 {
            *sample = f32_to_i16(scratch[0]);
            provided = provided.saturating_add(1);
        } else {
            *sample = 0;
        }
    }
    provided
}

fn write_u16_samples(data: &mut [u16], consumer: &mut HeapCons<f32>) -> usize {
    let mut provided = 0usize;
    let mut scratch = [0.0f32];
    for sample in data {
        if consumer.pop_slice(&mut scratch) == 1 {
            *sample = f32_to_u16(scratch[0]);
            provided = provided.saturating_add(1);
        } else {
            *sample = 0;
        }
    }
    provided
}

fn f32_to_i16(value: f32) -> i16 {
    let scaled = value.clamp(-1.0, 1.0) * i16::MAX as f32;
    scaled as i16
}

fn f32_to_u16(value: f32) -> u16 {
    let normalized = (value.clamp(-1.0, 1.0) + 1.0) * 0.5;
    (normalized * u16::MAX as f32) as u16
}
