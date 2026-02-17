use std::error::Error;
use std::io::{stdin, stdout, ErrorKind};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, Builder, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use stellatune_asio_proto::{
    read_frame, shm::SharedRingMapped, write_frame, AudioSpec, DeviceCaps, DeviceInfo, ProtoError,
    Request, Response, SampleFormat, SharedRingFile, PROTOCOL_VERSION,
};

#[cfg(windows)]
use windows::core::HSTRING;
#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::System::Threading::{
    AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority, GetCurrentProcess, SetPriorityClass,
    AVRT_PRIORITY_HIGH, HIGH_PRIORITY_CLASS,
};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = stdin();
    let stdout = stdout();
    let mut r = stdin.lock();
    let mut w = stdout.lock();

    #[cfg(windows)]
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
    }

    let mut state: Option<StreamState> = None;

    loop {
        let req: Request = match read_frame(&mut r) {
            Ok(v) => v,
            Err(e) => {
                // EOF / broken pipe => exit.
                if matches!(e, ProtoError::Io(ref io) if io.kind() == ErrorKind::UnexpectedEof) {
                    break;
                }
                let _ = write_frame(
                    &mut w,
                    &Response::Err {
                        message: e.to_string(),
                    },
                );
                continue;
            }
        };

        match req {
            Request::Hello { version } => {
                if version != PROTOCOL_VERSION {
                    write_frame(
                        &mut w,
                        &Response::Err {
                            message: format!(
                                "protocol version mismatch: client={version}, host={}",
                                PROTOCOL_VERSION
                            ),
                        },
                    )?;
                } else {
                    write_frame(&mut w, &Response::HelloOk { version })?;
                }
            }
            Request::ListDevices => {
                let devices = list_devices()?;
                write_frame(&mut w, &Response::Devices { devices })?;
            }
            Request::GetDeviceCaps { device_id } => {
                let caps = get_device_caps(&device_id)?;
                write_frame(&mut w, &Response::DeviceCaps { caps })?;
            }
            Request::Open {
                device_id,
                spec,
                buffer_size_frames,
                shared_ring,
            } => {
                state = Some(StreamState::open(
                    &device_id,
                    spec,
                    buffer_size_frames,
                    shared_ring,
                )?);
                write_frame(&mut w, &Response::Ok)?;
            }
            Request::Start => {
                if let Some(s) = state.as_ref() {
                    s.start()?;
                    write_frame(&mut w, &Response::Ok)?;
                } else {
                    write_frame(
                        &mut w,
                        &Response::Err {
                            message: "not opened".to_string(),
                        },
                    )?;
                }
            }
            Request::Stop => {
                let _ = state.take();
                write_frame(&mut w, &Response::Ok)?;
            }
            Request::Close => {
                let _ = state.take();
                write_frame(&mut w, &Response::Ok)?;
                break;
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
struct MmcssGuard(#[allow(dead_code)] HANDLE);

#[cfg(windows)]
unsafe impl Send for MmcssGuard {}

#[cfg(windows)]
#[derive(Default)]
struct MmcssState {
    attempted: bool,
    guard: Option<MmcssGuard>,
}

#[cfg(windows)]
impl MmcssState {
    fn ensure_pro_audio(&mut self, format_label: &str) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        self.guard = enable_mmcss_pro_audio();
        if self.guard.is_some() {
            eprintln!("asio host mmcss: Pro Audio enabled ({format_label})");
        } else {
            eprintln!("asio host mmcss: failed to enable Pro Audio ({format_label})");
        }
    }
}

#[cfg(windows)]
fn enable_mmcss_pro_audio() -> Option<MmcssGuard> {
    let mut task_index = 0u32;
    let task = HSTRING::from("Pro Audio");
    let handle = unsafe { AvSetMmThreadCharacteristicsW(&task, &mut task_index) }.ok()?;
    let _ = unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH) };
    Some(MmcssGuard(handle))
}

fn device_id_string(dev: &cpal::Device) -> String {
    dev.id()
        .ok()
        .map(|id| id.to_string())
        .or_else(|| dev.description().ok().map(|d| d.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

fn asio_host() -> Result<cpal::Host, String> {
    #[cfg(all(windows, feature = "asio"))]
    {
        return cpal::host_from_id(cpal::HostId::Asio).map_err(|e| e.to_string());
    }
    #[cfg(not(all(windows, feature = "asio")))]
    {
        Err("ASIO support not built (enable `stellatune-asio-host` feature `asio`)".to_string())
    }
}

fn list_devices() -> Result<Vec<DeviceInfo>, String> {
    let host = asio_host()?;
    let mut out = Vec::new();
    let devs = host.output_devices().map_err(|e| e.to_string())?;
    for dev in devs {
        let id = device_id_string(&dev);
        let name = dev
            .description()
            .ok()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "Unknown ASIO Device".to_string());
        out.push(DeviceInfo { id, name });
    }
    Ok(out)
}

fn get_device_caps(device_id: &str) -> Result<DeviceCaps, String> {
    let host = asio_host()?;
    let devs = host.output_devices().map_err(|e| e.to_string())?;
    let dev = devs
        .into_iter()
        .find(|d| device_id_string(d) == device_id)
        .ok_or_else(|| format!("device not found: {device_id}"))?;

    let default_cfg = dev.default_output_config().map_err(|e| e.to_string())?;
    let default_spec = AudioSpec {
        sample_rate: default_cfg.sample_rate(),
        channels: default_cfg.channels(),
    };

    let mut rates = Vec::new();
    let mut chans = Vec::new();
    let mut fmts = Vec::new();

    if let Ok(configs) = dev.supported_output_configs() {
        for cfg in configs {
            let min = cfg.min_sample_rate();
            let max = cfg.max_sample_rate();
            // Enumerate common rates within range (small list, but useful for "match track rate").
            for r in [
                8000u32, 11025, 16000, 22050, 32000, 44100, 48000, 88200, 96000, 176400, 192000,
            ] {
                if r >= min && r <= max {
                    rates.push(r);
                }
            }
            rates.push(min);
            rates.push(max);
            rates.push(default_spec.sample_rate);
            chans.push(cfg.channels());
            fmts.push(match cfg.sample_format() {
                cpal::SampleFormat::F32 => SampleFormat::F32,
                cpal::SampleFormat::I16 => SampleFormat::I16,
                cpal::SampleFormat::I32 => SampleFormat::I32,
                cpal::SampleFormat::U16 => SampleFormat::U16,
                _ => continue,
            });
        }
    }

    rates.sort_unstable();
    rates.dedup();
    chans.sort_unstable();
    chans.dedup();
    fmts.sort_unstable_by_key(|f| *f as u8);
    fmts.dedup();

    Ok(DeviceCaps {
        default_spec,
        supported_sample_rates: rates,
        supported_channels: chans,
        supported_formats: fmts,
    })
}

struct StreamState {
    running: Arc<AtomicBool>,
    metrics: Arc<UnderrunMetrics>,
    metrics_join: Option<JoinHandle<()>>,
    _stream: cpal::Stream,
}

impl StreamState {
    fn open(
        device_id: &str,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        shared_ring: Option<SharedRingFile>,
    ) -> Result<Self, String> {
        let host = asio_host()?;
        let devs = host.output_devices().map_err(|e| e.to_string())?;
        let dev = devs
            .into_iter()
            .find(|d| device_id_string(d) == device_id)
            .ok_or_else(|| format!("device not found: {device_id}"))?;

        let shared_ring = shared_ring.ok_or_else(|| "shared ring not provided".to_string())?;
        let ring_capacity_samples = shared_ring.capacity_samples;
        let ring_path = shared_ring.path;
        let ring = SharedRingMapped::open(Path::new(&ring_path))
            .map_err(|e| format!("failed to open shared ring: {e}"))?;
        if ring.capacity_samples() != ring_capacity_samples as usize {
            return Err("shared ring capacity mismatch".to_string());
        }
        if ring.channels() != spec.channels {
            return Err("shared ring channel mismatch".to_string());
        }
        ring.reset();

        let running = Arc::new(AtomicBool::new(false));
        let metrics = Arc::new(UnderrunMetrics::default());
        let metrics_join = Some(start_underrun_reporter(
            Arc::clone(&metrics),
            spec.sample_rate,
            spec.channels,
        ));

        let cfg = cpal::StreamConfig {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            buffer_size: match buffer_size_frames {
                Some(n) => cpal::BufferSize::Fixed(n),
                None => cpal::BufferSize::Default,
            },
        };

        // Prefer f32; if unavailable, fall back to i16/i32/u16.
        let supported = dev.supported_output_configs().map_err(|e| e.to_string())?;
        let mut chosen_format = None;
        for cand in [
            cpal::SampleFormat::F32,
            cpal::SampleFormat::I16,
            cpal::SampleFormat::I32,
            cpal::SampleFormat::U16,
        ] {
            if supported.clone().any(|c| c.sample_format() == cand) {
                chosen_format = Some(cand);
                break;
            }
        }
        let chosen_format = chosen_format.unwrap_or(cpal::SampleFormat::F32);

        let err_fn = |e| eprintln!("cpal stream error: {e}");

        let stream = match chosen_format {
            cpal::SampleFormat::F32 => {
                let ring = SharedRingMapped::open(Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                #[cfg(windows)]
                let mut mmcss_state = MmcssState::default();
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [f32], _| {
                        #[cfg(windows)]
                        mmcss_state.ensure_pro_audio("f32");
                        fill_shm_f32(out, &ring, &running_cb, &metrics_cb)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            cpal::SampleFormat::I16 => {
                let mut tmp = vec![0f32; 0];
                let ring = SharedRingMapped::open(Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                #[cfg(windows)]
                let mut mmcss_state = MmcssState::default();
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [i16], _| {
                        #[cfg(windows)]
                        mmcss_state.ensure_pro_audio("i16");
                        fill_shm_i16(out, &ring, &running_cb, &metrics_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            cpal::SampleFormat::I32 => {
                let mut tmp = vec![0f32; 0];
                let ring = SharedRingMapped::open(Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                #[cfg(windows)]
                let mut mmcss_state = MmcssState::default();
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [i32], _| {
                        #[cfg(windows)]
                        mmcss_state.ensure_pro_audio("i32");
                        fill_shm_i32(out, &ring, &running_cb, &metrics_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            cpal::SampleFormat::U16 => {
                let mut tmp = vec![0f32; 0];
                let ring = SharedRingMapped::open(Path::new(&ring_path))
                    .map_err(|e| format!("failed to open shared ring: {e}"))?;
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                #[cfg(windows)]
                let mut mmcss_state = MmcssState::default();
                dev.build_output_stream(
                    &cfg,
                    move |out: &mut [u16], _| {
                        #[cfg(windows)]
                        mmcss_state.ensure_pro_audio("u16");
                        fill_shm_u16(out, &ring, &running_cb, &metrics_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            }
            other => return Err(format!("unsupported sample format: {other:?}")),
        };

        Ok(Self {
            running,
            metrics,
            metrics_join,
            _stream: stream,
        })
    }

    fn start(&self) -> Result<(), String> {
        self.running.store(true, Ordering::Release);
        self._stream.play().map_err(|e| e.to_string())
    }
}

impl Drop for StreamState {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        self.metrics.stop.store(true, Ordering::Release);
        if let Some(join) = self.metrics_join.take() {
            let _ = join.join();
        }
    }
}

#[derive(Default)]
struct UnderrunMetrics {
    underrun_callbacks: AtomicU64,
    underrun_samples: AtomicU64,
    delivered_samples: AtomicU64,
    max_shortfall_samples: AtomicU64,
    stop: AtomicBool,
}

impl UnderrunMetrics {
    fn note_underrun_samples(&self, shortfall_samples: usize) {
        if shortfall_samples == 0 {
            return;
        }
        self.underrun_callbacks.fetch_add(1, Ordering::Relaxed);
        self.underrun_samples
            .fetch_add(shortfall_samples as u64, Ordering::Relaxed);
        let value = shortfall_samples as u64;
        let mut cur = self.max_shortfall_samples.load(Ordering::Relaxed);
        while value > cur {
            match self.max_shortfall_samples.compare_exchange(
                cur,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => cur = v,
            }
        }
    }
}

fn start_underrun_reporter(
    metrics: Arc<UnderrunMetrics>,
    sample_rate: u32,
    channels: u16,
) -> JoinHandle<()> {
    Builder::new()
        .name("stellatune-asio-underrun".to_string())
        .spawn(move || {
            let mut last_callbacks = 0u64;
            let mut last_samples = 0u64;
            let mut last_delivered = 0u64;
            while !metrics.stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_secs(1));
                let callbacks = metrics.underrun_callbacks.load(Ordering::Relaxed);
                let samples = metrics.underrun_samples.load(Ordering::Relaxed);
                let delivered = metrics.delivered_samples.load(Ordering::Relaxed);
                if callbacks <= last_callbacks {
                    continue;
                }
                let delta_callbacks = callbacks - last_callbacks;
                let delta_samples = samples.saturating_sub(last_samples);
                let delta_delivered = delivered.saturating_sub(last_delivered);
                last_callbacks = callbacks;
                last_samples = samples;
                last_delivered = delivered;

                // Suppress pause/idle spam: if no actual audio samples were delivered
                // in this interval, underrun callbacks are expected and not actionable.
                if delta_delivered == 0 {
                    continue;
                }

                let frames = delta_samples / channels.max(1) as u64;
                let delta_ms = if sample_rate > 0 {
                    (frames.saturating_mul(1000)) / sample_rate as u64
                } else {
                    0
                };
                let max_shortfall_samples = metrics.max_shortfall_samples.load(Ordering::Relaxed);
                let max_frames = max_shortfall_samples / channels.max(1) as u64;
                let max_shortfall_ms = if sample_rate > 0 {
                    (max_frames.saturating_mul(1000)) / sample_rate as u64
                } else {
                    0
                };

                eprintln!(
                    "asio underrun stats: +{} callbacks +{} samples (~{}ms) delivered_samples={} total_callbacks={} total_samples={} max_shortfall_samples={} (~{}ms)",
                    delta_callbacks,
                    delta_samples,
                    delta_ms,
                    delta_delivered,
                    callbacks,
                    samples,
                    max_shortfall_samples,
                    max_shortfall_ms
                );
            }
        })
        .expect("failed to spawn stellatune-asio-underrun thread")
}

fn read_from_ring_with_underrun(
    ring: &SharedRingMapped,
    out: &mut [f32],
    metrics: &Arc<UnderrunMetrics>,
) -> usize {
    let n = ring.read_samples(out);
    if n > 0 {
        metrics
            .delivered_samples
            .fetch_add(n as u64, Ordering::Relaxed);
    }
    if n < out.len() {
        metrics.note_underrun_samples(out.len() - n);
    }
    n
}

fn fill_shm_f32(
    out: &mut [f32],
    ring: &SharedRingMapped,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
) {
    if !running.load(Ordering::Acquire) {
        out.fill(0.0);
        return;
    }
    let n = read_from_ring_with_underrun(ring, out, metrics);
    if n < out.len() {
        out[n..].fill(0.0);
    }
}

fn ensure_tmp(tmp: &mut Vec<f32>, len: usize) {
    if tmp.len() < len {
        tmp.resize(len, 0.0);
    }
}

fn fill_shm_i16(
    out: &mut [i16],
    ring: &SharedRingMapped,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::Acquire) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = read_from_ring_with_underrun(ring, &mut tmp[..out.len()], metrics);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        *dst = (v * i16::MAX as f32) as i16;
    }
}

fn fill_shm_i32(
    out: &mut [i32],
    ring: &SharedRingMapped,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::Acquire) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = read_from_ring_with_underrun(ring, &mut tmp[..out.len()], metrics);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        *dst = (v * i32::MAX as f32) as i32;
    }
}

fn fill_shm_u16(
    out: &mut [u16],
    ring: &SharedRingMapped,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::Acquire) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = read_from_ring_with_underrun(ring, &mut tmp[..out.len()], metrics);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        let normalized = (v + 1.0) * 0.5;
        *dst = (normalized * u16::MAX as f32) as u16;
    }
}
