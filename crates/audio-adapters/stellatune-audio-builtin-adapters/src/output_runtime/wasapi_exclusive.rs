#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::Error;
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

use super::{AudioBackend, AudioDevice, OutputError, OutputSpec, SampleConsumer};

pub struct WasapiExclusiveHandle {
    shutdown: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

// MMCSS logic moved to super::mmcss

impl Drop for WasapiExclusiveHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn select_device(
    enumerator: &DeviceEnumerator,
    device_id: Option<&str>,
) -> Result<wasapi::Device, OutputError> {
    if device_id.is_none() {
        return enumerator
            .get_default_device(&Direction::Render)
            .map_err(|e| OutputError::ConfigMismatch {
                message: e.to_string(),
            });
    }
    let sel = device_id.expect("checked");
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|e| OutputError::ConfigMismatch {
            message: e.to_string(),
        })?;
    for dev in collection.into_iter().flatten() {
        if dev.get_id().ok().as_deref() == Some(sel) {
            return Ok(dev);
        }
    }
    Err(OutputError::NoDevice)
}

pub fn supports_exclusive_spec(
    device_id: Option<String>,
    spec: OutputSpec,
) -> Result<bool, OutputError> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new().map_err(|e| OutputError::ConfigMismatch {
        message: e.to_string(),
    })?;
    let device = select_device(&enumerator, device_id.as_deref())?;
    let audio_client = device
        .get_iaudioclient()
        .map_err(|e| OutputError::ConfigMismatch {
            message: e.to_string(),
        })?;

    let requested = [
        WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            spec.sample_rate as usize,
            spec.channels as usize,
            None,
        ),
        WaveFormat::new(
            16,
            16,
            &SampleType::Int,
            spec.sample_rate as usize,
            spec.channels as usize,
            None,
        ),
        WaveFormat::new(
            32,
            32,
            &SampleType::Int,
            spec.sample_rate as usize,
            spec.channels as usize,
            None,
        ),
    ];

    for fmt in requested {
        if audio_client
            .is_supported_exclusive_with_quirks(&fmt)
            .is_ok()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn output_spec_for_exclusive_device(
    device_id: Option<String>,
) -> Result<OutputSpec, OutputError> {
    let _ = wasapi::initialize_mta();

    let enumerator = DeviceEnumerator::new().map_err(|e| OutputError::ConfigMismatch {
        message: e.to_string(),
    })?;

    let device = select_device(&enumerator, device_id.as_deref())?;

    let audio_client = device
        .get_iaudioclient()
        .map_err(|e| OutputError::ConfigMismatch {
            message: e.to_string(),
        })?;

    let mix = audio_client
        .get_mixformat()
        .map_err(|e| OutputError::ConfigMismatch {
            message: e.to_string(),
        })?;

    Ok(OutputSpec {
        sample_rate: mix.get_samplespersec(),
        channels: mix.get_nchannels(),
    })
}

impl WasapiExclusiveHandle {
    pub fn start<C: SampleConsumer, F>(
        device_id: Option<String>,
        mut consumer: C,
        expected_spec: OutputSpec,
        on_error: F,
    ) -> Result<Self, OutputError>
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);

        let thread = thread::Builder::new()
            .name("stellatune-wasapi-exclusive".to_string())
            .spawn(move || {
                #[cfg(windows)]
                let _mmcss = super::mmcss::enable_mmcss_pro_audio();
                if let Err(e) =
                    run_exclusive_loop(device_id, &mut consumer, expected_spec, thread_shutdown)
                {
                    on_error(e.to_string());
                }
            })
            .map_err(|e| OutputError::ConfigMismatch {
                message: format!("failed to spawn wasapi-exclusive thread: {e}"),
            })?;

        Ok(Self {
            shutdown,
            thread: Some(thread),
        })
    }

    pub fn stop(self) {
        drop(self);
    }
}

fn run_exclusive_loop<C: SampleConsumer>(
    device_id: Option<String>,
    consumer: &mut C,
    expected_spec: OutputSpec,
    shutdown: Arc<AtomicBool>,
) -> Result<(), Error> {
    let _ = wasapi::initialize_mta();

    let enumerator = DeviceEnumerator::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let device =
        select_device(&enumerator, device_id.as_deref()).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut audio_client = device
        .get_iaudioclient()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let mix = audio_client.get_mixformat().ok();

    #[derive(Debug, Clone, Copy)]
    enum RenderSampleKind {
        F32,
        I16,
        I32,
    }

    let requested_formats = [
        (
            WaveFormat::new(
                32,
                32,
                &SampleType::Float,
                expected_spec.sample_rate as usize,
                expected_spec.channels as usize,
                None,
            ),
            RenderSampleKind::F32,
        ),
        (
            WaveFormat::new(
                16,
                16,
                &SampleType::Int,
                expected_spec.sample_rate as usize,
                expected_spec.channels as usize,
                None,
            ),
            RenderSampleKind::I16,
        ),
        (
            WaveFormat::new(
                32,
                32,
                &SampleType::Int,
                expected_spec.sample_rate as usize,
                expected_spec.channels as usize,
                None,
            ),
            RenderSampleKind::I32,
        ),
    ];

    let mut last_err = None;
    let mut selected = None;
    for (fmt, kind) in requested_formats {
        match audio_client.is_supported_exclusive_with_quirks(&fmt) {
            Ok(supported) => {
                selected = Some((supported, kind));
                break;
            },
            Err(e) => last_err = Some(anyhow::anyhow!("{e}")),
        }
    }

    let (format, sample_kind) = selected.ok_or_else(|| {
        let dev_label = device_id.as_deref().unwrap_or("default");
        let mut details = format!(
            "Could not find a compatible format (exclusive): requested {}Hz {}ch for device \"{}\"",
            expected_spec.sample_rate, expected_spec.channels, dev_label
        );
        if let Some(mix) = &mix {
            let sub = mix.get_subformat().ok();
            details.push_str(&format!(
                "; mixformat = {}Hz {}ch {:?} {}bits",
                mix.get_samplespersec(),
                mix.get_nchannels(),
                sub,
                mix.get_bitspersample(),
            ));
        }
        if let Some(e) = last_err {
            details.push_str(&format!("; last error: {e}"));
        }
        anyhow::anyhow!(details)
    })?;

    let period_100ns = match audio_client.get_device_period() {
        Ok((default_hns, min_hns)) => default_hns.max(min_hns),
        Err(_) => wasapi::calculate_period_100ns(
            (expected_spec.sample_rate / 100) as i64, // ~10ms fallback
            expected_spec.sample_rate as i64,
        ),
    };
    // Use polling mode for exclusive output. Event-driven exclusive mode is known to cause
    // stuttering on some USB audio devices/drivers.
    let buffer_duration_hns = period_100ns.saturating_mul(4).max(period_100ns);
    let mode = StreamMode::PollingExclusive {
        buffer_duration_hns,
        period_hns: period_100ns,
    };

    audio_client
        .initialize_client(&format, &Direction::Render, &mode)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let render_client = audio_client
        .get_audiorenderclient()
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    let buffer_frame_count = audio_client
        .get_buffer_size()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    audio_client
        .start_stream()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let channels = expected_spec.channels as usize;
    let bytes_per_frame = format.get_blockalign() as usize;
    let max_samples = (buffer_frame_count as usize) * channels;

    fn f32_to_i16(v: f32) -> i16 {
        let v = v.clamp(-1.0, 1.0);
        (v * i16::MAX as f32) as i16
    }

    fn f32_to_i32(v: f32) -> i32 {
        let v = v.clamp(-1.0, 1.0);
        (v * i32::MAX as f32) as i32
    }

    let mut f32_buf = if matches!(sample_kind, RenderSampleKind::F32) {
        Some(vec![0f32; max_samples])
    } else {
        None
    };
    let mut i16_buf = if matches!(sample_kind, RenderSampleKind::I16) {
        Some(vec![0i16; max_samples])
    } else {
        None
    };
    let mut i32_buf = if matches!(sample_kind, RenderSampleKind::I32) {
        Some(vec![0i32; max_samples])
    } else {
        None
    };

    while !shutdown.load(Ordering::Relaxed) {
        let available_frames = audio_client
            .get_available_space_in_frames()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        if available_frames == 0 {
            // Nothing to write. Sleep briefly to avoid busy looping.
            thread::sleep(Duration::from_millis(1));
            continue;
        }

        if available_frames > 0 {
            let frames = available_frames as usize;
            let samples_needed = frames * channels;
            let nbr_bytes = frames * bytes_per_frame;

            let mut provided = 0usize;
            let bytes: &[u8] = match sample_kind {
                RenderSampleKind::F32 => {
                    let buf = f32_buf.as_mut().expect("f32 buffer");
                    for s in &mut buf[..samples_needed] {
                        if let Some(v) = consumer.pop_sample() {
                            *s = v;
                            provided += 1;
                        } else {
                            *s = 0.0;
                        }
                    }
                    consumer.on_output(samples_needed, provided);
                    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, nbr_bytes) }
                },
                RenderSampleKind::I16 => {
                    let buf = i16_buf.as_mut().expect("i16 buffer");
                    for s in &mut buf[..samples_needed] {
                        if let Some(v) = consumer.pop_sample() {
                            *s = f32_to_i16(v);
                            provided += 1;
                        } else {
                            *s = 0;
                        }
                    }
                    consumer.on_output(samples_needed, provided);
                    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, nbr_bytes) }
                },
                RenderSampleKind::I32 => {
                    let buf = i32_buf.as_mut().expect("i32 buffer");
                    for s in &mut buf[..samples_needed] {
                        if let Some(v) = consumer.pop_sample() {
                            *s = f32_to_i32(v);
                            provided += 1;
                        } else {
                            *s = 0;
                        }
                    }
                    consumer.on_output(samples_needed, provided);
                    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, nbr_bytes) }
                },
            };

            render_client
                .write_to_device(frames, bytes, None)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
    }

    let _ = audio_client.stop_stream();
    Ok(())
}

pub fn list_exclusive_devices_detailed() -> Result<Vec<AudioDevice>, OutputError> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new().map_err(|e| OutputError::ConfigMismatch {
        message: e.to_string(),
    })?;
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|e| OutputError::ConfigMismatch {
            message: e.to_string(),
        })?;
    let mut devices = Vec::new();
    for dev in collection.into_iter().flatten() {
        let id = dev.get_id().unwrap_or_else(|_| "unknown".to_string());
        let name = dev
            .get_friendlyname()
            .unwrap_or_else(|_| "Unknown WASAPI Device".to_string());
        devices.push(AudioDevice {
            backend: AudioBackend::WasapiExclusive,
            id,
            name,
        });
    }
    Ok(devices)
}
