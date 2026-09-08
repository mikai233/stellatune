use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, Builder, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::traits::{Consumer as _, Observer as _, Producer as _, Split as _};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use stellatune_asio_proto::AudioSpec;

use crate::device::find_live_device;
use crate::platform::stream::OutputCallbackPlatformState;

const DEFAULT_QUEUE_MS: u32 = 80;
const MIN_QUEUE_MS: u32 = 20;
const MIN_QUEUE_FRAMES: u32 = 1024;
const MAX_QUEUE_SAMPLES: usize = 4 * 1024 * 1024;

struct LocalSampleQueue {
    producer: Mutex<HeapProd<f32>>,
    consumer: Mutex<HeapCons<f32>>,
}

impl LocalSampleQueue {
    /// Only used after opening, before playback can enter a callback.
    fn ensure_capacity(&self, capacity: usize) {
        let mut producer = self.producer.lock().unwrap();
        if producer.capacity().get() >= capacity {
            return;
        }
        let mut consumer = self.consumer.lock().unwrap();
        let (next_producer, next_consumer) = HeapRb::<f32>::new(capacity).split();
        *producer = next_producer;
        *consumer = next_consumer;
    }
    fn new(capacity_samples: usize) -> Self {
        let rb = HeapRb::<f32>::new(capacity_samples.max(1));
        let (producer, consumer) = rb.split();
        Self {
            producer: Mutex::new(producer),
            consumer: Mutex::new(consumer),
        }
    }

    fn write_samples(&self, input: &[f32]) -> usize {
        let Ok(mut producer) = self.producer.lock() else {
            return 0;
        };
        producer.push_slice(input)
    }

    fn read_samples(&self, out: &mut [f32]) -> usize {
        let Ok(mut consumer) = self.consumer.lock() else {
            return 0;
        };
        consumer.pop_slice(out)
    }

    fn queued_samples(&self) -> u32 {
        let Ok(producer) = self.producer.lock() else {
            return 0;
        };
        producer.occupied_len().min(u32::MAX as usize) as u32
    }

    fn reset(&self) {
        if let Ok(mut consumer) = self.consumer.lock() {
            let _ = consumer.clear();
        }
    }
}

#[derive(Clone)]
pub(crate) struct StreamIngress {
    queue: Arc<LocalSampleQueue>,
    channels: u16,
}

impl StreamIngress {
    pub(crate) fn bytes_per_frame(&self) -> usize {
        self.channels.max(1) as usize * std::mem::size_of::<f32>()
    }

    pub(crate) fn write_interleaved_f32le(&self, interleaved_f32le: &[u8]) -> Result<u32, String> {
        if !interleaved_f32le
            .len()
            .is_multiple_of(std::mem::size_of::<f32>())
        {
            return Err("interleaved_f32le length must be a multiple of 4".to_string());
        }

        let channels = self.channels.max(1) as usize;
        let sample_count = interleaved_f32le.len() / std::mem::size_of::<f32>();
        if !sample_count.is_multiple_of(channels) {
            return Err("samples not aligned to channels".to_string());
        }

        let mut samples = Vec::<f32>::with_capacity(sample_count);
        for bytes in interleaved_f32le.as_chunks::<4>().0 {
            samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        }

        let accepted_samples = self.queue.write_samples(samples.as_slice());
        Ok((accepted_samples / channels) as u32)
    }
}

pub(crate) struct StreamState {
    running: Arc<AtomicBool>,
    queue: Arc<LocalSampleQueue>,
    channels: u16,
    metrics: Arc<UnderrunMetrics>,
    metrics_join: Option<JoinHandle<()>>,
    _stream: cpal::Stream,
}

impl StreamState {
    pub(crate) fn open(
        device_id: &str,
        spec: AudioSpec,
        buffer_size_frames: Option<u32>,
        queue_capacity_ms: Option<u32>,
    ) -> Result<Self, String> {
        let dev = find_live_device(device_id)?;
        let channels = spec.channels.max(1);
        let queue = Arc::new(LocalSampleQueue::new(queue_capacity_samples(
            &spec,
            buffer_size_frames,
            queue_capacity_ms,
        )));
        queue.reset();

        let running = Arc::new(AtomicBool::new(false));
        let metrics = Arc::new(UnderrunMetrics::default());

        let cfg = cpal::StreamConfig {
            channels,
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
            if supported.clone().any(|c| {
                c.sample_format() == cand
                    && c.channels() == channels
                    && c.min_sample_rate() <= spec.sample_rate
                    && c.max_sample_rate() >= spec.sample_rate
            }) {
                chosen_format = Some(cand);
                break;
            }
        }
        let chosen_format = chosen_format.ok_or_else(|| {
            "ASIO device does not support the requested rate/channel format".to_string()
        })?;

        let error_metrics = Arc::clone(&metrics);
        let err_fn = move |e| {
            error_metrics.failed.store(true, Ordering::Release);
            tracing::error!("cpal stream error: {e}");
        };

        let stream = match chosen_format {
            cpal::SampleFormat::F32 => {
                let queue_cb = Arc::clone(&queue);
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                let mut platform_state = OutputCallbackPlatformState::new();
                dev.build_output_stream(
                    cfg,
                    move |out: &mut [f32], _| {
                        let _callback = metrics_cb.enter();
                        platform_state.on_callback_start("f32");
                        fill_queue_f32(out, &queue_cb, &running_cb, &metrics_cb)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            },
            cpal::SampleFormat::I16 => {
                let mut tmp = vec![0f32; 0];
                let queue_cb = Arc::clone(&queue);
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                let mut platform_state = OutputCallbackPlatformState::new();
                dev.build_output_stream(
                    cfg,
                    move |out: &mut [i16], _| {
                        let _callback = metrics_cb.enter();
                        platform_state.on_callback_start("i16");
                        fill_queue_i16(out, &queue_cb, &running_cb, &metrics_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            },
            cpal::SampleFormat::I32 => {
                let mut tmp = vec![0f32; 0];
                let queue_cb = Arc::clone(&queue);
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                let mut platform_state = OutputCallbackPlatformState::new();
                dev.build_output_stream(
                    cfg,
                    move |out: &mut [i32], _| {
                        let _callback = metrics_cb.enter();
                        platform_state.on_callback_start("i32");
                        fill_queue_i32(out, &queue_cb, &running_cb, &metrics_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            },
            cpal::SampleFormat::U16 => {
                let mut tmp = vec![0f32; 0];
                let queue_cb = Arc::clone(&queue);
                let running_cb = Arc::clone(&running);
                let metrics_cb = Arc::clone(&metrics);
                let mut platform_state = OutputCallbackPlatformState::new();
                dev.build_output_stream(
                    cfg,
                    move |out: &mut [u16], _| {
                        let _callback = metrics_cb.enter();
                        platform_state.on_callback_start("u16");
                        fill_queue_u16(out, &queue_cb, &running_cb, &metrics_cb, &mut tmp)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
            },
            other => return Err(format!("unsupported sample format: {other:?}")),
        };

        let hardware_frames = stream.buffer_size().unwrap_or(MIN_QUEUE_FRAMES) as usize;
        queue.ensure_capacity((hardware_frames * 2 * usize::from(channels)).min(MAX_QUEUE_SAMPLES));
        // CPAL's ASIO pause skips callbacks without stopping the driver or
        // clearing its double buffers. Keep callbacks active even while our
        // playback gate is closed so both hardware buffers receive silence.
        stream.play().map_err(|error| error.to_string())?;
        let metrics_join = Some(start_underrun_reporter(
            Arc::clone(&metrics),
            spec.sample_rate,
            channels,
        ));
        Ok(Self {
            running,
            queue,
            channels,
            metrics,
            metrics_join,
            _stream: stream,
        })
    }

    pub(crate) fn start(&self) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        if let Err(error) = self._stream.play() {
            self.running.store(false, Ordering::SeqCst);
            return Err(error.to_string());
        }
        Ok(())
    }

    pub(crate) fn pause(&self) -> Result<(), String> {
        // A single order between admission and in-flight counting prevents a
        // new callback from slipping past the zero-count check during reset.
        self.running.store(false, Ordering::SeqCst);
        // Do not call cpal::Stream::pause(): the driver would repeat old PCM.
        // Closed-gate callbacks write silence and leave queued music untouched.
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        while self.metrics.callbacks_in_flight.load(Ordering::SeqCst) != 0 {
            if std::time::Instant::now() >= deadline {
                return Err("ASIO callback did not quiesce".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }

    pub(crate) fn buffer_size_frames(&self) -> u32 {
        self._stream.buffer_size().unwrap_or(0)
    }
    pub(crate) fn failed(&self) -> bool {
        self.metrics.failed.load(Ordering::Acquire)
    }

    pub(crate) fn consumed_frames(&self) -> u64 {
        self.metrics.delivered_samples.load(Ordering::Acquire) / u64::from(self.channels)
    }

    pub(crate) fn reset(&self) {
        self.queue.reset();
    }

    pub(crate) fn queued_samples(&self) -> u32 {
        self.queue.queued_samples()
    }

    pub(crate) fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub(crate) fn ingress(&self) -> StreamIngress {
        StreamIngress {
            queue: Arc::clone(&self.queue),
            channels: self.channels,
        }
    }

    pub(crate) fn write_interleaved_f32le(&self, interleaved_f32le: &[u8]) -> Result<u32, String> {
        self.ingress().write_interleaved_f32le(interleaved_f32le)
    }
}

impl Drop for StreamState {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.metrics.stop.store(true, Ordering::Release);
        if let Some(join) = self.metrics_join.take() {
            join.thread().unpark();
            let _ = join.join();
        }
    }
}

fn queue_capacity_samples(
    spec: &AudioSpec,
    buffer_size_frames: Option<u32>,
    queue_capacity_ms: Option<u32>,
) -> usize {
    let channels = spec.channels.max(1) as u64;
    let sample_rate = spec.sample_rate.max(1) as u64;
    let queue_ms = queue_capacity_ms
        .unwrap_or(DEFAULT_QUEUE_MS)
        .max(MIN_QUEUE_MS) as u64;
    let by_time_frames = sample_rate.saturating_mul(queue_ms) / 1000;
    let by_buffer_frames = buffer_size_frames
        .unwrap_or(MIN_QUEUE_FRAMES)
        .max(MIN_QUEUE_FRAMES)
        .saturating_mul(2) as u64;
    let frames = by_time_frames
        .max(by_buffer_frames)
        .max(MIN_QUEUE_FRAMES as u64);
    let min_samples = channels.saturating_mul(MIN_QUEUE_FRAMES as u64);
    let samples = frames.saturating_mul(channels).max(min_samples);
    samples.min(MAX_QUEUE_SAMPLES as u64).min(usize::MAX as u64) as usize
}

#[derive(Default)]
struct UnderrunMetrics {
    callbacks_in_flight: AtomicU64,
    failed: AtomicBool,
    underrun_callbacks: AtomicU64,
    underrun_samples: AtomicU64,
    delivered_samples: AtomicU64,
    max_shortfall_samples: AtomicU64,
    stop: AtomicBool,
}

impl UnderrunMetrics {
    fn enter(&self) -> CallbackGuard<'_> {
        self.callbacks_in_flight.fetch_add(1, Ordering::SeqCst);
        CallbackGuard(self)
    }
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

struct CallbackGuard<'a>(&'a UnderrunMetrics);
impl Drop for CallbackGuard<'_> {
    fn drop(&mut self) {
        self.0.callbacks_in_flight.fetch_sub(1, Ordering::SeqCst);
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
                thread::park_timeout(Duration::from_secs(1));
                if metrics.stop.load(Ordering::Acquire) {
                    break;
                }
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

                tracing::warn!(
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

fn read_from_queue_with_underrun(
    queue: &Arc<LocalSampleQueue>,
    out: &mut [f32],
    metrics: &Arc<UnderrunMetrics>,
) -> usize {
    let n = queue.read_samples(out);
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

fn fill_queue_f32(
    out: &mut [f32],
    queue: &Arc<LocalSampleQueue>,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
) {
    if !running.load(Ordering::SeqCst) {
        out.fill(0.0);
        return;
    }
    let n = read_from_queue_with_underrun(queue, out, metrics);
    if n < out.len() {
        out[n..].fill(0.0);
    }
}

fn ensure_tmp(tmp: &mut Vec<f32>, len: usize) {
    if tmp.len() < len {
        tmp.resize(len, 0.0);
    }
}

fn fill_queue_i16(
    out: &mut [i16],
    queue: &Arc<LocalSampleQueue>,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::SeqCst) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = read_from_queue_with_underrun(queue, &mut tmp[..out.len()], metrics);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        *dst = (v * i16::MAX as f32) as i16;
    }
}

fn fill_queue_i32(
    out: &mut [i32],
    queue: &Arc<LocalSampleQueue>,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::SeqCst) {
        out.fill(0);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = read_from_queue_with_underrun(queue, &mut tmp[..out.len()], metrics);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        *dst = (v * i32::MAX as f32) as i32;
    }
}

fn fill_queue_u16(
    out: &mut [u16],
    queue: &Arc<LocalSampleQueue>,
    running: &Arc<AtomicBool>,
    metrics: &Arc<UnderrunMetrics>,
    tmp: &mut Vec<f32>,
) {
    if !running.load(Ordering::SeqCst) {
        out.fill(1 << 15);
        return;
    }
    ensure_tmp(tmp, out.len());
    let n = read_from_queue_with_underrun(queue, &mut tmp[..out.len()], metrics);
    if n < out.len() {
        tmp[n..out.len()].fill(0.0);
    }
    for (dst, src) in out.iter_mut().zip(tmp.iter()) {
        let v = src.clamp(-1.0, 1.0);
        *dst = ((v * 32768.0) + 32768.0).clamp(0.0, u16::MAX as f32) as u16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_callbacks_clear_both_buffers_without_consuming_music() {
        let queue = Arc::new(LocalSampleQueue::new(16));
        let running = Arc::new(AtomicBool::new(false));
        let metrics = Arc::new(UnderrunMetrics::default());
        let music = [0.25, -0.25, 0.5, -0.5];
        queue.write_samples(&music);
        let mut float_buffers = [[0.9; 4]; 2];
        let mut tmp = Vec::new();
        for buffer in &mut float_buffers {
            fill_queue_f32(buffer, &queue, &running, &metrics);
            assert_eq!(*buffer, [0.0; 4]);
            let mut i16_buffer = [123; 4];
            let mut i32_buffer = [123; 4];
            let mut u16_buffer = [123; 4];
            fill_queue_i16(&mut i16_buffer, &queue, &running, &metrics, &mut tmp);
            fill_queue_i32(&mut i32_buffer, &queue, &running, &metrics, &mut tmp);
            fill_queue_u16(&mut u16_buffer, &queue, &running, &metrics, &mut tmp);
            assert_eq!(i16_buffer, [0; 4]);
            assert_eq!(i32_buffer, [0; 4]);
            assert_eq!(u16_buffer, [32768; 4]);
        }
        assert_eq!(queue.queued_samples(), 4);
        assert_eq!(metrics.delivered_samples.load(Ordering::Acquire), 0);
        assert_eq!(metrics.underrun_callbacks.load(Ordering::Acquire), 0);
        running.store(true, Ordering::SeqCst);
        fill_queue_f32(&mut float_buffers[0], &queue, &running, &metrics);
        assert_eq!(float_buffers[0], music);
        assert_eq!(metrics.delivered_samples.load(Ordering::Acquire), 4);
        // Underrun must also overwrite previously rendered samples with silence.
        fill_queue_f32(&mut float_buffers[0], &queue, &running, &metrics);
        assert_eq!(float_buffers[0], [0.0; 4]);
    }

    #[test]
    fn unsigned_pcm_silence_is_the_same_when_paused_playing_or_starved() {
        let queue = Arc::new(LocalSampleQueue::new(16));
        let running = Arc::new(AtomicBool::new(true));
        let metrics = Arc::new(UnderrunMetrics::default());
        queue.write_samples(&[0.0, 0.0]);
        let mut buffer = [0; 4];
        fill_queue_u16(&mut buffer, &queue, &running, &metrics, &mut Vec::new());
        assert_eq!(buffer, [32768; 4]);
    }
}
