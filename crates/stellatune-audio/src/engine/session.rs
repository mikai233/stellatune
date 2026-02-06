use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use crossbeam_channel::Sender;
use tracing::{debug, info};

use stellatune_core::TrackDecodeInfo;
use stellatune_output::{OutputError, OutputHandle, OutputSpec};

use crate::engine::config::{
    BUFFER_PREFILL_CAP_MS, BUFFER_PREFILL_CAP_MS_EXCLUSIVE, RING_BUFFER_CAPACITY_MS,
};
use crate::engine::decode::decode_thread;
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{DecodeCtrl, InternalMsg};
use crate::ring_buffer::{RingBufferConsumer, RingBufferProducer, new_ring_buffer};

const OUTPUT_CONSUMER_CHUNK_SAMPLES: usize = 1024;

pub(crate) struct OutputPipeline {
    pub(crate) _output: OutputHandle,
    pub(crate) producer: Arc<Mutex<RingBufferProducer<f32>>>,
    pub(crate) output_enabled: Arc<AtomicBool>,
    pub(crate) buffered_samples: Arc<AtomicUsize>,
    pub(crate) underrun_callbacks: Arc<AtomicU64>,
    pub(crate) backend: stellatune_output::AudioBackend,
    pub(crate) device_id: Option<String>,
    pub(crate) out_sample_rate: u32,
    pub(crate) out_channels: u16,
}

pub(crate) struct PlaybackSession {
    pub(crate) ctrl_tx: Sender<DecodeCtrl>,
    pub(crate) decode_join: JoinHandle<()>,
    pub(crate) output_enabled: Arc<AtomicBool>,
    pub(crate) buffered_samples: Arc<AtomicUsize>,
    pub(crate) underrun_callbacks: Arc<AtomicU64>,
    pub(crate) out_sample_rate: u32,
    pub(crate) out_channels: u16,
    pub(crate) track_info: TrackDecodeInfo,
}

fn create_output_pipeline(
    backend: stellatune_output::AudioBackend,
    device_id: Option<String>,
    out_spec: OutputSpec,
    volume: Arc<AtomicU32>,
    internal_tx: Sender<InternalMsg>,
) -> Result<OutputPipeline, String> {
    let capacity_samples =
        ((out_spec.sample_rate as usize * out_spec.channels as usize * RING_BUFFER_CAPACITY_MS)
            / 1000)
            .max(1024);
    let (producer, consumer) = new_ring_buffer::<f32>(capacity_samples);
    let producer = Arc::new(Mutex::new(producer));
    let consumer = Arc::new(Mutex::new(consumer));

    let output_enabled = Arc::new(AtomicBool::new(false));
    let buffered_samples = Arc::new(AtomicUsize::new(0));
    let underrun_callbacks = Arc::new(AtomicU64::new(0));

    let output_consumer = GatedConsumer {
        inner: Arc::clone(&consumer),
        enabled: Arc::clone(&output_enabled),
        volume,
        buffered_samples: Arc::clone(&buffered_samples),
        underrun_callbacks: Arc::clone(&underrun_callbacks),
        scratch: vec![0.0; OUTPUT_CONSUMER_CHUNK_SAMPLES],
        scratch_len: 0,
        scratch_cursor: 0,
    };

    let output = OutputHandle::start(
        backend,
        device_id.clone(),
        output_consumer,
        out_spec,
        move |err| {
            let _ = internal_tx.try_send(InternalMsg::OutputError(err.to_string()));
        },
    )
    .map_err(|e| match e {
        OutputError::ConfigMismatch { message } => message,
        other => other.to_string(),
    })?;

    Ok(OutputPipeline {
        _output: output,
        producer,
        output_enabled,
        buffered_samples,
        underrun_callbacks,
        backend,
        device_id,
        out_sample_rate: out_spec.sample_rate,
        out_channels: out_spec.channels,
    })
}

pub(crate) fn start_session(
    path: String,
    events: Arc<EventHub>,
    internal_tx: Sender<InternalMsg>,
    backend: stellatune_output::AudioBackend,
    device_id: Option<String>,
    match_track_sample_rate: bool,
    gapless_playback: bool,
    out_spec: OutputSpec,
    start_at_ms: i64,
    volume: Arc<AtomicU32>,
    plugins: Arc<Mutex<stellatune_plugins::PluginManager>>,
    lfe_mode: stellatune_core::LfeMode,
    output_pipeline: &mut Option<OutputPipeline>,
) -> Result<PlaybackSession, String> {
    let t0 = Instant::now();
    debug!(%path, start_at_ms, "start_session begin");
    info!("starting session");
    let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded();
    let (setup_tx, setup_rx) = crossbeam_channel::bounded::<DecodeCtrl>(1);
    let (spec_tx, spec_rx) = crossbeam_channel::bounded::<Result<TrackDecodeInfo, String>>(1);

    let thread_path = path.clone();
    let thread_events = Arc::clone(&events);
    let thread_internal_tx = internal_tx.clone();
    let thread_plugins = Arc::clone(&plugins);

    let decode_join = std::thread::Builder::new()
        .name("stellatune-decode".to_string())
        .spawn(move || {
            decode_thread(
                thread_path,
                thread_events,
                thread_internal_tx,
                thread_plugins,
                ctrl_rx,
                setup_rx,
                spec_tx,
            )
        })
        .expect("failed to spawn stellatune-decode thread");

    let t_after_spawn = Instant::now();
    let track_info = match spec_rx.recv() {
        Ok(Ok(info)) => info,
        Ok(Err(message)) => return Err(message),
        Err(_) => return Err("decoder thread exited unexpectedly".to_string()),
    };
    debug!(
        "decoder opened/probed in {}ms",
        t_after_spawn.elapsed().as_millis()
    );

    let mut desired_out_spec = out_spec;
    if match_track_sample_rate
        && matches!(
            backend,
            stellatune_output::AudioBackend::WasapiExclusive
                | stellatune_output::AudioBackend::Asio
        )
    {
        let candidate = OutputSpec {
            sample_rate: track_info.sample_rate,
            channels: out_spec.channels,
        };
        if stellatune_output::supports_output_spec(backend, device_id.clone(), candidate) {
            desired_out_spec = candidate;
        }
    }

    let target_spec = if let Some(existing) = output_pipeline.as_ref() {
        let existing_spec = OutputSpec {
            sample_rate: existing.out_sample_rate,
            channels: existing.out_channels,
        };
        let same_route =
            existing.backend == backend && existing.device_id.as_deref() == device_id.as_deref();
        let can_reuse = same_route
            && if gapless_playback {
                existing_spec.channels == desired_out_spec.channels
            } else {
                existing_spec.sample_rate == desired_out_spec.sample_rate
                    && existing_spec.channels == desired_out_spec.channels
            };
        if can_reuse {
            existing_spec
        } else {
            desired_out_spec
        }
    } else {
        desired_out_spec
    };

    let rebuild_pipeline = output_pipeline
        .as_ref()
        .map(|p| {
            p.backend != backend
                || p.device_id.as_deref() != device_id.as_deref()
                || p.out_sample_rate != target_spec.sample_rate
                || p.out_channels != target_spec.channels
        })
        .unwrap_or(true);
    if rebuild_pipeline {
        *output_pipeline = Some(create_output_pipeline(
            backend,
            device_id,
            target_spec,
            Arc::clone(&volume),
            internal_tx.clone(),
        )?);
    }

    let pipeline = output_pipeline
        .as_ref()
        .ok_or_else(|| "output pipeline not initialized".to_string())?;
    if let Ok(mut producer) = pipeline.producer.lock() {
        producer.clear();
    }

    setup_tx
        .send(DecodeCtrl::Setup {
            producer: Arc::clone(&pipeline.producer),
            target_sample_rate: pipeline.out_sample_rate,
            target_channels: pipeline.out_channels,
            start_at_ms,
            output_enabled: Arc::clone(&pipeline.output_enabled),
            buffer_prefill_cap_ms: match backend {
                stellatune_output::AudioBackend::WasapiExclusive => BUFFER_PREFILL_CAP_MS_EXCLUSIVE,
                _ => BUFFER_PREFILL_CAP_MS,
            },
            lfe_mode,
        })
        .map_err(|_| "decoder thread exited unexpectedly".to_string())?;

    debug!("start_session total {}ms", t0.elapsed().as_millis());
    Ok(PlaybackSession {
        ctrl_tx,
        decode_join,
        output_enabled: Arc::clone(&pipeline.output_enabled),
        buffered_samples: Arc::clone(&pipeline.buffered_samples),
        underrun_callbacks: Arc::clone(&pipeline.underrun_callbacks),
        out_sample_rate: pipeline.out_sample_rate,
        out_channels: pipeline.out_channels,
        track_info,
    })
}

struct GatedConsumer {
    inner: Arc<Mutex<RingBufferConsumer<f32>>>,
    enabled: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    buffered_samples: Arc<AtomicUsize>,
    underrun_callbacks: Arc<AtomicU64>,
    scratch: Vec<f32>,
    scratch_len: usize,
    scratch_cursor: usize,
}

impl stellatune_output::SampleConsumer for GatedConsumer {
    fn pop_sample(&mut self) -> Option<f32> {
        if !self.enabled.load(Ordering::Acquire) {
            return None;
        }

        if self.scratch_cursor >= self.scratch_len {
            self.scratch_cursor = 0;
            self.scratch_len = if let Ok(mut inner) = self.inner.lock() {
                inner.pop_slice(&mut self.scratch)
            } else {
                0
            };
            if self.scratch_len == 0 {
                return None;
            }
        }

        let sample = self.scratch[self.scratch_cursor];
        self.scratch_cursor += 1;
        let v = f32::from_bits(self.volume.load(Ordering::Relaxed));
        Some(sample * v)
    }

    fn on_output(&mut self, requested: usize, provided: usize) {
        let buffered = self.inner.lock().map(|inner| inner.len()).unwrap_or(0);
        self.buffered_samples.store(buffered, Ordering::Relaxed);
        if self.enabled.load(Ordering::Relaxed) && provided < requested {
            self.underrun_callbacks.fetch_add(1, Ordering::Relaxed);
        }
    }
}
