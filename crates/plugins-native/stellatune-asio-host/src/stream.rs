use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, Builder, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, StreamTrait};
use stellatune_asio_proto::{AudioSpec, SharedRingFile, shm::SharedRingMapped};

use crate::device::find_live_device;

#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::System::Threading::{
    AVRT_PRIORITY_HIGH, AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority,
};
#[cfg(windows)]
use windows::core::HSTRING;

pub(crate) struct StreamState {
    running: Arc<AtomicBool>,
    metrics: Arc<UnderrunMetrics>,
    metrics_join: Option<JoinHandle<()>>,
    _stream: cpal::Stream,
}

impl StreamState {
    pub(crate) fn open(
        device_id: &str,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        shared_ring: Option<SharedRingFile>,
    ) -> Result<Self, String> {
        let dev = find_live_device(device_id)?;

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
            },
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
            },
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
            },
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
            },
            other => return Err(format!("unsupported sample format: {other:?}")),
        };

        Ok(Self {
            running,
            metrics,
            metrics_join,
            _stream: stream,
        })
    }

    pub(crate) fn start(&self) -> Result<(), String> {
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
