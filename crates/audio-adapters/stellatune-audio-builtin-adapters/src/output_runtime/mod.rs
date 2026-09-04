#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use stellatune_audio_core::format::{ChannelLayout, SpeakerPosition};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSpec {
    pub sample_rate: u32,
    pub channel_layout: ChannelLayout,
}

impl OutputSpec {
    pub fn channel_count(self) -> u16 {
        self.channel_layout.channel_count()
    }
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
    DefaultConfig(cpal::Error),

    #[error("unsupported stream config: {0}")]
    StreamConfig(cpal::Error),

    #[error("failed to build output stream: {0}")]
    BuildStream(cpal::Error),

    #[error("failed to play output stream: {0}")]
    PlayStream(cpal::Error),

    #[error("failed to pause output stream: {0}")]
    PauseStream(cpal::Error),

    #[error("output device config mismatch: {message}")]
    ConfigMismatch { message: String },

    #[error("failed to query devices: {0}")]
    Devices(cpal::Error),

    #[error("unknown or unsupported output channel layout: {message}")]
    ChannelLayout { message: String },
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
    Exclusive {
        handle: wasapi_exclusive::WasapiExclusiveHandle,
        spec: OutputSpec,
    },
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

        let mut counts = HashMap::new();
        for d in &devs {
            *counts.entry(d.name.clone()).or_insert(0) += 1;
        }

        let mut final_devs = Vec::new();
        let mut current_indices = HashMap::new();
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
        AudioBackend::Shared => {
            output_spec_for_device(backend, device_id).is_ok_and(|actual| actual == spec)
        },
        #[cfg(windows)]
        AudioBackend::WasapiExclusive => {
            wasapi_exclusive::supports_exclusive_spec(device_id, spec).unwrap_or(false)
        },
        #[cfg(not(windows))]
        AudioBackend::WasapiExclusive => {
            let _ = (device_id, spec);
            false
        },
    }
}

fn cpal_device_label(device: &cpal::Device) -> String {
    match device.description() {
        Ok(desc) => {
            // On Windows (WASAPI), `desc.name()` is often a generic endpoint label (e.g. "Speakers")
            // while the more user-recognizable name (e.g. "Speakers (SMSL USB DAC)") is stored in
            // `desc.extended()`.
            desc.extended()
                .next()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| desc.name().trim())
                .to_string()
        },
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
    output_spec_from_shared_device(&device)
}

pub fn output_spec_for_device(
    backend: AudioBackend,
    device_id: Option<String>,
) -> Result<OutputSpec, OutputError> {
    match backend {
        AudioBackend::Shared => {
            let host = cpal::default_host();
            let device = if let Some(sel) = device_id {
                host.output_devices()
                    .map_err(OutputError::Devices)?
                    .find(|d| cpal_device_id(d) == sel)
                    .ok_or(OutputError::NoDevice)?
            } else {
                host.default_output_device().ok_or(OutputError::NoDevice)?
            };
            output_spec_from_shared_device(&device)
        },
        #[cfg(windows)]
        AudioBackend::WasapiExclusive => {
            wasapi_exclusive::output_spec_for_wasapi_device(device_id.as_deref())
        },
        #[cfg(not(windows))]
        AudioBackend::WasapiExclusive => Err(OutputError::NoDevice),
    }
}

fn output_spec_from_shared_device(device: &cpal::Device) -> Result<OutputSpec, OutputError> {
    let config = device
        .default_output_config()
        .map_err(OutputError::DefaultConfig)?;

    #[cfg(windows)]
    {
        let device_id = device.id().map_err(OutputError::Devices)?;
        let spec = wasapi_exclusive::output_spec_for_wasapi_device(Some(device_id.id()))?;
        if spec.sample_rate != config.sample_rate() || spec.channel_count() != config.channels() {
            return Err(OutputError::ChannelLayout {
                message: format!(
                    "CPAL default config {}Hz/{}ch differs from WASAPI mix format {}Hz/{}ch",
                    config.sample_rate(),
                    config.channels(),
                    spec.sample_rate,
                    spec.channel_count(),
                ),
            });
        }
        Ok(spec)
    }

    #[cfg(not(windows))]
    {
        Ok(OutputSpec {
            sample_rate: config.sample_rate(),
            channel_layout: infer_unpositioned_layout(config.channels())?,
        })
    }
}

fn infer_unpositioned_layout(channels: u16) -> Result<ChannelLayout, OutputError> {
    match channels {
        1 => Ok(ChannelLayout::MONO),
        2 => Ok(ChannelLayout::STEREO),
        _ => Err(OutputError::ChannelLayout {
            message: format!("backend reported {channels} channels without speaker positions"),
        }),
    }
}

#[cfg(windows)]
fn channel_layout_from_standard_mask(
    channels: u16,
    mask: u32,
) -> Result<ChannelLayout, OutputError> {
    if mask == 0 {
        return infer_unpositioned_layout(channels);
    }
    let known_mask = (1_u32 << SpeakerPosition::ALL.len()) - 1;
    if mask & !known_mask != 0 {
        return Err(OutputError::ChannelLayout {
            message: format!("WASAPI channel mask contains unsupported bits: {mask:#010x}"),
        });
    }
    if mask.count_ones() != u32::from(channels) {
        return Err(OutputError::ChannelLayout {
            message: format!(
                "WASAPI channel mask/count mismatch: mask={mask:#010x}, channels={channels}"
            ),
        });
    }
    ChannelLayout::from_positions(
        SpeakerPosition::ALL
            .into_iter()
            .filter(|position| mask & (1_u32 << *position as u8) != 0),
    )
    .map_err(|error| OutputError::ChannelLayout {
        message: error.to_string(),
    })
}

#[cfg(windows)]
fn standard_mask_from_channel_layout(layout: ChannelLayout) -> u32 {
    layout
        .positions()
        .fold(0_u32, |mask, position| mask | (1_u32 << position as u8))
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
                    host.output_devices()
                        .map_err(OutputError::Devices)?
                        .find(|d| cpal_device_id(d) == sel)
                        .ok_or(OutputError::NoDevice)?
                } else {
                    host.default_output_device().ok_or(OutputError::NoDevice)?
                };

                let config = device
                    .default_output_config()
                    .map_err(OutputError::DefaultConfig)?;
                let sample_rate = config.sample_rate();
                let channels = config.channels();
                let actual_spec = output_spec_from_shared_device(&device)?;

                if actual_spec != expected_spec {
                    return Err(OutputError::ConfigMismatch {
                        message: format!(
                            "format mismatch: expected = {expected_spec:?}, output = {actual_spec:?}"
                        ),
                    });
                }

                debug_assert_eq!(sample_rate, actual_spec.sample_rate);
                debug_assert_eq!(channels, actual_spec.channel_count());

                let stream_config: cpal::StreamConfig = config.into();
                let on_error = Arc::new(on_error);

                let stream = match config.sample_format() {
                    cpal::SampleFormat::F32 => {
                        let on_error = Arc::clone(&on_error);
                        device
                            .build_output_stream(
                                stream_config,
                                move |data: &mut [f32], _| {
                                    #[cfg(windows)]
                                    let _ = mmcss::ensure_mmcss_pro_audio_for_current_thread();
                                    fill_f32(data, &mut consumer)
                                },
                                move |err| (on_error)(err.to_string()),
                                Some(Duration::from_millis(200)),
                            )
                            .map_err(OutputError::BuildStream)?
                    },
                    cpal::SampleFormat::I16 => {
                        let on_error = Arc::clone(&on_error);
                        device
                            .build_output_stream(
                                stream_config,
                                move |data: &mut [i16], _| {
                                    #[cfg(windows)]
                                    let _ = mmcss::ensure_mmcss_pro_audio_for_current_thread();
                                    fill_i16(data, &mut consumer)
                                },
                                move |err| (on_error)(err.to_string()),
                                Some(Duration::from_millis(200)),
                            )
                            .map_err(OutputError::BuildStream)?
                    },
                    cpal::SampleFormat::U16 => {
                        let on_error = Arc::clone(&on_error);
                        device
                            .build_output_stream(
                                stream_config,
                                move |data: &mut [u16], _| {
                                    #[cfg(windows)]
                                    let _ = mmcss::ensure_mmcss_pro_audio_for_current_thread();
                                    fill_u16(data, &mut consumer)
                                },
                                move |err| (on_error)(err.to_string()),
                                Some(Duration::from_millis(200)),
                            )
                            .map_err(OutputError::BuildStream)?
                    },
                    other => {
                        return Err(OutputError::ConfigMismatch {
                            message: format!("unsupported output sample format: {other:?}"),
                        });
                    },
                };

                stream.play().map_err(OutputError::PlayStream)?;

                Ok(Self::Shared {
                    _stream: stream,
                    spec: actual_spec,
                })
            },
            #[cfg(windows)]
            AudioBackend::WasapiExclusive => {
                let handle = wasapi_exclusive::WasapiExclusiveHandle::start(
                    device_id,
                    consumer,
                    expected_spec,
                    on_error,
                )?;
                Ok(Self::Exclusive {
                    handle,
                    spec: expected_spec,
                })
            },
            #[cfg(not(windows))]
            AudioBackend::WasapiExclusive => Err(OutputError::NoDevice),
        }
    }

    pub fn spec(&self) -> OutputSpec {
        match self {
            Self::Shared { spec, .. } => *spec,
            #[cfg(windows)]
            Self::Exclusive { spec, .. } => *spec,
        }
    }

    pub fn pause(&self) -> Result<(), OutputError> {
        match self {
            Self::Shared { _stream, .. } => _stream.pause().map_err(OutputError::PauseStream),
            #[cfg(windows)]
            Self::Exclusive { handle, .. } => {
                handle.pause();
                Ok(())
            },
        }
    }

    pub fn resume(&self) -> Result<(), OutputError> {
        match self {
            Self::Shared { _stream, .. } => _stream.play().map_err(OutputError::PlayStream),
            #[cfg(windows)]
            Self::Exclusive { handle, .. } => {
                handle.resume();
                Ok(())
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
            },
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
            },
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
            },
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

#[cfg(all(test, windows))]
mod tests {
    use stellatune_audio_core::format::ChannelLayout;

    use super::{channel_layout_from_standard_mask, standard_mask_from_channel_layout};

    #[test]
    fn standard_channel_masks_round_trip() {
        for layout in [
            ChannelLayout::MONO,
            ChannelLayout::STEREO,
            ChannelLayout::QUAD,
            ChannelLayout::SURROUND_5_1_SIDE,
            ChannelLayout::SURROUND_5_1_REAR,
            ChannelLayout::SURROUND_7_1,
            ChannelLayout::SURROUND_7_1_4,
        ] {
            let mask = standard_mask_from_channel_layout(layout);
            assert_eq!(
                channel_layout_from_standard_mask(layout.channel_count(), mask).unwrap(),
                layout
            );
        }
    }

    #[test]
    fn zero_mask_is_only_inferred_for_mono_and_stereo() {
        assert_eq!(
            channel_layout_from_standard_mask(1, 0).unwrap(),
            ChannelLayout::MONO
        );
        assert_eq!(
            channel_layout_from_standard_mask(2, 0).unwrap(),
            ChannelLayout::STEREO
        );
        assert!(channel_layout_from_standard_mask(6, 0).is_err());
    }

    #[test]
    fn rejects_mask_count_mismatch_and_unknown_bits() {
        assert!(channel_layout_from_standard_mask(5, 0x060f).is_err());
        assert!(channel_layout_from_standard_mask(1, 1 << 20).is_err());
    }
}
