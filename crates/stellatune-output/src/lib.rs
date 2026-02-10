use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

pub trait SampleConsumer: Send + 'static {
    fn pop_sample(&mut self) -> Option<f32>;

    /// Called once per audio callback after the output buffer has been filled.
    ///
    /// `requested` is the number of samples the callback needed, `provided` is the number of
    /// samples actually obtained from the ring buffer.
    ///
    /// This must be lightweight (no allocations/locks/IO).
    fn on_output(&mut self, _requested: usize, _provided: usize) {}
}

#[derive(Debug, Error)]
pub enum OutputError {
    #[error("no default output device")]
    NoDevice,

    #[error("failed to query default output config: {0}")]
    DefaultConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("unsupported stream config: {0}")]
    StreamConfig(#[from] cpal::SupportedStreamConfigsError),

    #[error("failed to build output stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),

    #[error("failed to play output stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),

    #[error("output device config mismatch: {message}")]
    ConfigMismatch { message: String },

    #[error("failed to query devices: {0}")]
    Devices(#[from] cpal::DevicesError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AudioBackend {
    Shared,
    WasapiExclusive,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDevice {
    pub backend: AudioBackend,
    pub id: String,
    pub name: String,
}

#[cfg(windows)]
mod wasapi_exclusive;

#[cfg(windows)]
pub(crate) mod mmcss;

/// Best-effort realtime hint for audio-critical worker threads.
///
/// On Windows this enables MMCSS "Pro Audio" for the current thread and keeps
/// it active for the guard lifetime. On other platforms it is a no-op.
pub struct RealtimeThreadGuard {
    #[cfg(windows)]
    _mmcss: Option<mmcss::MmcssGuard>,
}

pub fn enable_realtime_audio_thread() -> RealtimeThreadGuard {
    #[cfg(windows)]
    {
        RealtimeThreadGuard {
            _mmcss: mmcss::enable_mmcss_pro_audio(),
        }
    }
    #[cfg(not(windows))]
    {
        RealtimeThreadGuard {}
    }
}

pub enum OutputHandle {
    Shared {
        _stream: cpal::Stream,
        spec: OutputSpec,
    },
    #[cfg(windows)]
    Exclusive(wasapi_exclusive::WasapiExclusiveHandle),
}

pub fn list_host_devices(_selected_backend: Option<AudioBackend>) -> Vec<AudioDevice> {
    let mut shared_devices = Vec::new();

    // CPAL Shared Output
    let host = cpal::default_host();
    if let Ok(cpal_devs) = host.output_devices() {
        for device in cpal_devs {
            let name = cpal_device_label(&device);
            shared_devices.push(AudioDevice {
                backend: AudioBackend::Shared,
                id: cpal_device_id(&device),
                name,
            });
        }
    }

    let exclusive_devices = {
        #[cfg(windows)]
        {
            wasapi_exclusive::list_exclusive_devices_detailed().unwrap_or_default()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    };

    // Helper to sort and disambiguate a list of devices
    fn process_list(mut devs: Vec<AudioDevice>) -> Vec<AudioDevice> {
        // Sort by name for stable indexing
        devs.sort_by(|a, b| a.name.cmp(&b.name));

        let mut counts = std::collections::HashMap::new();
        for d in &devs {
            *counts.entry(d.name.clone()).or_insert(0) += 1;
        }

        let mut final_devs = Vec::new();
        let mut current_indices = std::collections::HashMap::new();

        for d in devs {
            let count = counts[&d.name];
            if count > 1 {
                let idx = current_indices.entry(d.name.clone()).or_insert(0);
                *idx += 1;
                final_devs.push(AudioDevice {
                    backend: d.backend,
                    id: d.id,
                    name: format!("{} ({})", d.name, idx),
                });
            } else {
                final_devs.push(d);
            }
        }
        final_devs
    }

    let mut all_devices = process_list(shared_devices);
    all_devices.extend(process_list(exclusive_devices));

    all_devices
}

pub fn supports_output_spec(
    backend: AudioBackend,
    device_id: Option<String>,
    spec: OutputSpec,
) -> bool {
    match backend {
        AudioBackend::Shared => true,
        #[cfg(windows)]
        AudioBackend::WasapiExclusive => {
            wasapi_exclusive::supports_exclusive_spec(device_id, spec).unwrap_or(false)
        }
        #[cfg(not(windows))]
        AudioBackend::WasapiExclusive => {
            let _ = (device_id, spec);
            false
        }
    }
}

fn cpal_device_label(device: &cpal::Device) -> String {
    match device.description() {
        Ok(desc) => {
            // On Windows (WASAPI), `desc.name()` is often a generic endpoint label (e.g. "Speakers")
            // while the more user-recognizable name (e.g. "Speakers (SMSL USB DAC)") is stored in
            // `desc.extended()`.
            desc.extended()
                .first()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| desc.name().trim())
                .to_string()
        }
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

pub fn default_output_spec() -> Result<OutputSpec, OutputError> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(OutputError::NoDevice)?;
    let config = device.default_output_config()?;
    Ok(OutputSpec {
        sample_rate: config.sample_rate(),
        channels: config.channels(),
    })
}

pub fn output_spec_for_device(
    backend: AudioBackend,
    device_id: Option<String>,
) -> Result<OutputSpec, OutputError> {
    match backend {
        AudioBackend::Shared => {
            let host = cpal::default_host();
            let device = if let Some(sel) = device_id {
                host.output_devices()?
                    .find(|d| cpal_device_id(d) == sel)
                    .ok_or(OutputError::NoDevice)?
            } else {
                host.default_output_device().ok_or(OutputError::NoDevice)?
            };
            let config = device.default_output_config()?;
            Ok(OutputSpec {
                sample_rate: config.sample_rate(),
                channels: config.channels(),
            })
        }
        #[cfg(windows)]
        AudioBackend::WasapiExclusive => {
            wasapi_exclusive::output_spec_for_exclusive_device(device_id)
        }
        #[cfg(not(windows))]
        AudioBackend::WasapiExclusive => Err(OutputError::NoDevice),
    }
}

impl OutputHandle {
    pub fn start<C: SampleConsumer, F>(
        backend: AudioBackend,
        device_id: Option<String>,
        mut consumer: C,
        expected_spec: OutputSpec,
        on_error: F,
    ) -> Result<Self, OutputError>
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        match backend {
            AudioBackend::Shared => {
                let host = cpal::default_host();
                let device = if let Some(sel) = device_id {
                    host.output_devices()?
                        .find(|d| cpal_device_id(d) == sel)
                        .ok_or(OutputError::NoDevice)?
                } else {
                    host.default_output_device().ok_or(OutputError::NoDevice)?
                };

                let config = device.default_output_config()?;
                let sample_rate = config.sample_rate();
                let channels = config.channels();

                if sample_rate != expected_spec.sample_rate {
                    return Err(OutputError::ConfigMismatch {
                        message: format!(
                            "sample rate mismatch: expected = {}Hz, output = {sample_rate}Hz",
                            expected_spec.sample_rate
                        ),
                    });
                }
                if channels != expected_spec.channels {
                    return Err(OutputError::ConfigMismatch {
                        message: format!(
                            "channel mismatch: expected = {}ch, output = {channels}ch",
                            expected_spec.channels
                        ),
                    });
                }

                let spec = OutputSpec {
                    sample_rate,
                    channels,
                };

                let stream_config: cpal::StreamConfig = config.clone().into();
                let on_error = Arc::new(on_error);

                let stream = match config.sample_format() {
                    cpal::SampleFormat::F32 => {
                        let on_error = Arc::clone(&on_error);
                        device.build_output_stream(
                            &stream_config,
                            move |data: &mut [f32], _| fill_f32(data, &mut consumer),
                            move |err| (on_error)(err.to_string()),
                            Some(Duration::from_millis(200)),
                        )?
                    }
                    cpal::SampleFormat::I16 => {
                        let on_error = Arc::clone(&on_error);
                        device.build_output_stream(
                            &stream_config,
                            move |data: &mut [i16], _| fill_i16(data, &mut consumer),
                            move |err| (on_error)(err.to_string()),
                            Some(Duration::from_millis(200)),
                        )?
                    }
                    cpal::SampleFormat::U16 => {
                        let on_error = Arc::clone(&on_error);
                        device.build_output_stream(
                            &stream_config,
                            move |data: &mut [u16], _| fill_u16(data, &mut consumer),
                            move |err| (on_error)(err.to_string()),
                            Some(Duration::from_millis(200)),
                        )?
                    }
                    other => {
                        return Err(OutputError::ConfigMismatch {
                            message: format!("unsupported output sample format: {other:?}"),
                        });
                    }
                };

                stream.play()?;

                Ok(Self::Shared {
                    _stream: stream,
                    spec,
                })
            }
            #[cfg(windows)]
            AudioBackend::WasapiExclusive => {
                let handle = wasapi_exclusive::WasapiExclusiveHandle::start(
                    device_id,
                    consumer,
                    expected_spec,
                    on_error,
                )?;
                Ok(Self::Exclusive(handle))
            }
            #[cfg(not(windows))]
            AudioBackend::WasapiExclusive => Err(OutputError::NoDevice),
        }
    }

    pub fn spec(&self) -> OutputSpec {
        match self {
            Self::Shared { spec, .. } => *spec,
            #[cfg(windows)]
            Self::Exclusive(_) => OutputSpec {
                sample_rate: 0, // Not easily available without storing it
                channels: 2,
            },
        }
    }
}

fn fill_f32<C: SampleConsumer>(out: &mut [f32], consumer: &mut C) {
    let mut provided = 0usize;
    for slot in out.iter_mut() {
        match consumer.pop_sample() {
            Some(v) => {
                provided += 1;
                *slot = v;
            }
            None => *slot = 0.0,
        }
    }
    consumer.on_output(out.len(), provided);
}

fn fill_i16<C: SampleConsumer>(out: &mut [i16], consumer: &mut C) {
    let mut provided = 0usize;
    for slot in out.iter_mut() {
        match consumer.pop_sample() {
            Some(v) => {
                provided += 1;
                *slot = f32_to_i16(v);
            }
            None => *slot = 0,
        }
    }
    consumer.on_output(out.len(), provided);
}

fn fill_u16<C: SampleConsumer>(out: &mut [u16], consumer: &mut C) {
    let mut provided = 0usize;
    for slot in out.iter_mut() {
        match consumer.pop_sample() {
            Some(v) => {
                provided += 1;
                *slot = f32_to_u16(v);
            }
            None => *slot = 0,
        }
    }
    consumer.on_output(out.len(), provided);
}

fn f32_to_i16(v: f32) -> i16 {
    let v = v.clamp(-1.0, 1.0);
    (v * i16::MAX as f32) as i16
}

fn f32_to_u16(v: f32) -> u16 {
    let v = v.clamp(-1.0, 1.0);
    let normalized = (v + 1.0) * 0.5;
    (normalized * u16::MAX as f32) as u16
}
