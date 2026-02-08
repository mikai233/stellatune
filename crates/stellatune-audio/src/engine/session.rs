use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use crossbeam_channel::{Receiver, Sender};
use tracing::{debug, warn};

use stellatune_core::TrackDecodeInfo;
use stellatune_output::{OutputError, OutputHandle, OutputSpec};

use crate::engine::config::{
    BUFFER_PREFILL_CAP_MS, BUFFER_PREFILL_CAP_MS_EXCLUSIVE, RING_BUFFER_CAPACITY_MS,
    SEEK_TRACK_FADE_RAMP_MS,
};
use crate::engine::decode::decoder::EngineDecoder;
use crate::engine::decode::{DecodeThreadArgs, decode_thread};
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{
    DecodeCtrl, DecodeWorkerState, InternalMsg, OutputSinkWrite, PredecodedChunk,
};
use crate::ring_buffer::{RingBufferConsumer, RingBufferProducer, new_ring_buffer};

const OUTPUT_CONSUMER_CHUNK_SAMPLES: usize = 1024;
#[cfg(debug_assertions)]
const DEBUG_SESSION_LOG_EVERY: u64 = 12;
#[cfg(debug_assertions)]
const DEBUG_PROMOTE_LOG_EVERY: u64 = 24;

mod debug_metrics {
    #[cfg(debug_assertions)]
    use super::DEBUG_PROMOTE_LOG_EVERY;
    #[cfg(debug_assertions)]
    use super::DEBUG_SESSION_LOG_EVERY;
    #[cfg(debug_assertions)]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(debug_assertions)]
    use tracing::debug;

    #[cfg(debug_assertions)]
    static WORKER_STARTS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static SESSION_STARTS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PREPARE_WAIT_TOTAL_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PREPARE_WAIT_MAX_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static SESSION_TOTAL_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static SESSION_TOTAL_MAX_MS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PIPELINE_REBUILDS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PROMOTE_STORES: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PROMOTE_LOOKUPS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PROMOTE_HITS: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PROMOTE_MISS_EMPTY: AtomicU64 = AtomicU64::new(0);
    #[cfg(debug_assertions)]
    static PROMOTE_MISS_MISMATCH: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug, Clone, Copy)]
    pub(crate) enum PromoteLookupResult {
        Hit,
        MissEmpty,
        MissMismatch,
    }

    #[cfg(debug_assertions)]
    fn update_max(max: &AtomicU64, value: u64) {
        let mut cur = max.load(Ordering::Relaxed);
        while value > cur {
            match max.compare_exchange(cur, value, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(v) => cur = v,
            }
        }
    }

    #[cfg(debug_assertions)]
    pub(crate) fn note_worker_start() {
        let starts = WORKER_STARTS.fetch_add(1, Ordering::Relaxed) + 1;
        debug!(starts, "decode worker started");
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_worker_start() {}

    #[cfg(debug_assertions)]
    pub(crate) fn note_promote_store() {
        PROMOTE_STORES.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_promote_store() {}

    #[cfg(debug_assertions)]
    pub(crate) fn note_promote_lookup(result: PromoteLookupResult) {
        let lookups = PROMOTE_LOOKUPS.fetch_add(1, Ordering::Relaxed) + 1;
        match result {
            PromoteLookupResult::Hit => {
                PROMOTE_HITS.fetch_add(1, Ordering::Relaxed);
            }
            PromoteLookupResult::MissEmpty => {
                PROMOTE_MISS_EMPTY.fetch_add(1, Ordering::Relaxed);
            }
            PromoteLookupResult::MissMismatch => {
                PROMOTE_MISS_MISMATCH.fetch_add(1, Ordering::Relaxed);
            }
        }
        if lookups.is_multiple_of(DEBUG_PROMOTE_LOG_EVERY) {
            let stores = PROMOTE_STORES.load(Ordering::Relaxed);
            let hits = PROMOTE_HITS.load(Ordering::Relaxed);
            let miss_empty = PROMOTE_MISS_EMPTY.load(Ordering::Relaxed);
            let miss_mismatch = PROMOTE_MISS_MISMATCH.load(Ordering::Relaxed);
            let hit_ratio = if lookups > 0 {
                hits as f64 / lookups as f64
            } else {
                0.0
            };
            debug!(
                stores,
                lookups, hits, miss_empty, miss_mismatch, hit_ratio, "decode promote stats"
            );
        }
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_promote_lookup(_result: PromoteLookupResult) {}

    #[cfg(debug_assertions)]
    pub(crate) fn note_session_start(
        prepare_wait_ms: u64,
        session_total_ms: u64,
        pipeline_rebuilt: bool,
    ) {
        let starts = SESSION_STARTS.fetch_add(1, Ordering::Relaxed) + 1;
        PREPARE_WAIT_TOTAL_MS.fetch_add(prepare_wait_ms, Ordering::Relaxed);
        SESSION_TOTAL_MS.fetch_add(session_total_ms, Ordering::Relaxed);
        if pipeline_rebuilt {
            PIPELINE_REBUILDS.fetch_add(1, Ordering::Relaxed);
        }
        update_max(&PREPARE_WAIT_MAX_MS, prepare_wait_ms);
        update_max(&SESSION_TOTAL_MAX_MS, session_total_ms);

        if starts.is_multiple_of(DEBUG_SESSION_LOG_EVERY) {
            let avg_prepare = PREPARE_WAIT_TOTAL_MS.load(Ordering::Relaxed) as f64 / starts as f64;
            let avg_total = SESSION_TOTAL_MS.load(Ordering::Relaxed) as f64 / starts as f64;
            let rebuilds = PIPELINE_REBUILDS.load(Ordering::Relaxed);
            debug!(
                starts,
                avg_prepare_ms = avg_prepare,
                max_prepare_ms = PREPARE_WAIT_MAX_MS.load(Ordering::Relaxed),
                avg_total_ms = avg_total,
                max_total_ms = SESSION_TOTAL_MAX_MS.load(Ordering::Relaxed),
                pipeline_rebuilds = rebuilds,
                "decode session stats"
            );
        }
    }

    #[cfg(not(debug_assertions))]
    pub(crate) fn note_session_start(
        _prepare_wait_ms: u64,
        _session_total_ms: u64,
        _pipeline_rebuilt: bool,
    ) {
    }
}

pub(crate) struct OutputPipeline {
    pub(crate) _output: Option<OutputHandle>,
    pub(crate) producer: Arc<Mutex<RingBufferProducer<f32>>>,
    pub(crate) output_enabled: Arc<AtomicBool>,
    pub(crate) buffered_samples: Arc<AtomicUsize>,
    pub(crate) underrun_callbacks: Arc<AtomicU64>,
    pub(crate) transition_gain: Arc<AtomicU32>,
    pub(crate) transition_target_gain: Arc<AtomicU32>,
    pub(crate) backend: stellatune_output::AudioBackend,
    pub(crate) device_id: Option<String>,
    pub(crate) device_output_enabled: bool,
    pub(crate) out_sample_rate: u32,
    pub(crate) out_channels: u16,
}

pub(crate) struct PlaybackSession {
    pub(crate) ctrl_tx: Sender<DecodeCtrl>,
    pub(crate) output_enabled: Arc<AtomicBool>,
    pub(crate) buffered_samples: Arc<AtomicUsize>,
    pub(crate) underrun_callbacks: Arc<AtomicU64>,
    pub(crate) transition_gain: Arc<AtomicU32>,
    pub(crate) transition_target_gain: Arc<AtomicU32>,
    pub(crate) out_sample_rate: u32,
    pub(crate) out_channels: u16,
    pub(crate) track_info: TrackDecodeInfo,
}

pub(crate) struct OutputSinkWorker {
    tx: Sender<OutputSinkWrite>,
    join: JoinHandle<()>,
}

impl OutputSinkWorker {
    pub(crate) fn start(mut sink: stellatune_plugins::OutputSinkInstance, channels: u16) -> Self {
        let (tx, rx) = crossbeam_channel::bounded::<OutputSinkWrite>(8);
        let join = std::thread::Builder::new()
            .name("stellatune-output-sink".to_string())
            .spawn(move || {
                while let Ok(msg) = rx.recv() {
                    match msg {
                        OutputSinkWrite::Samples(samples) => {
                            if samples.is_empty() {
                                continue;
                            }
                            if let Err(e) = write_all_frames(&mut sink, channels, &samples) {
                                warn!("output sink write failed: {e:#}");
                            }
                        }
                        OutputSinkWrite::Shutdown => {
                            let _ = sink.flush();
                            break;
                        }
                    }
                }
            })
            .expect("failed to spawn stellatune-output-sink thread");
        Self { tx, join }
    }

    pub(crate) fn sender(&self) -> Sender<OutputSinkWrite> {
        self.tx.clone()
    }

    pub(crate) fn shutdown(self) {
        let _ = self.tx.send(OutputSinkWrite::Shutdown);
        let _ = self.join.join();
    }
}

fn write_all_frames(
    sink: &mut stellatune_plugins::OutputSinkInstance,
    channels: u16,
    samples: &[f32],
) -> Result<(), String> {
    let channels = channels.max(1) as usize;
    if channels == 0 || samples.is_empty() {
        return Ok(());
    }
    let mut offset = 0usize;
    while offset < samples.len() {
        let frames_accepted = sink
            .write_interleaved_f32(channels as u16, &samples[offset..])
            .map_err(|e| e.to_string())?;
        let accepted_samples = frames_accepted as usize * channels;
        if accepted_samples == 0 {
            break;
        }
        offset = offset.saturating_add(accepted_samples.min(samples.len() - offset));
    }
    Ok(())
}

pub(crate) struct PromotedPreload {
    pub(crate) path: String,
    pub(crate) position_ms: u64,
    pub(crate) decoder: Box<EngineDecoder>,
    pub(crate) track_info: TrackDecodeInfo,
    pub(crate) chunk: PredecodedChunk,
}

struct DecodePrepare {
    path: String,
    producer: Arc<Mutex<RingBufferProducer<f32>>>,
    target_sample_rate: u32,
    target_channels: u16,
    start_at_ms: i64,
    output_enabled: Arc<AtomicBool>,
    buffer_prefill_cap_ms: i64,
    lfe_mode: stellatune_core::LfeMode,
    output_sink_only: bool,
    spec_tx: Sender<Result<TrackDecodeInfo, String>>,
}

enum DecodePrepareMsg {
    Prepare(DecodePrepare),
    Shutdown,
}

pub(crate) struct DecodeWorker {
    pub(crate) ctrl_tx: Sender<DecodeCtrl>,
    prepare_tx: Sender<DecodePrepareMsg>,
    promoted_preload: Arc<Mutex<Option<PromotedPreload>>>,
    join: JoinHandle<()>,
}

impl DecodeWorker {
    pub(crate) fn peek_promoted_track_info(
        &self,
        path: &str,
        position_ms: u64,
    ) -> Option<TrackDecodeInfo> {
        let Ok(slot) = self.promoted_preload.lock() else {
            return None;
        };
        let promoted = slot.as_ref()?;
        if promoted.path == path && promoted.position_ms == position_ms {
            return Some(promoted.track_info.clone());
        }
        None
    }

    pub(crate) fn promote_preload(&self, preload: PromotedPreload) {
        if let Ok(mut slot) = self.promoted_preload.lock() {
            *slot = Some(preload);
            debug_metrics::note_promote_store();
        }
    }

    pub(crate) fn shutdown(self) {
        let _ = self.ctrl_tx.send(DecodeCtrl::Stop);
        let _ = self.prepare_tx.send(DecodePrepareMsg::Shutdown);
        let _ = self.join.join();
    }
}

pub(crate) fn start_decode_worker(
    events: Arc<EventHub>,
    internal_tx: Sender<InternalMsg>,
    plugins: Arc<Mutex<stellatune_plugins::PluginManager>>,
) -> DecodeWorker {
    debug_metrics::note_worker_start();
    let runtime_state = Arc::new(AtomicU8::new(DecodeWorkerState::Idle as u8));
    let promoted_preload = Arc::new(Mutex::new(None));
    let promoted_preload_for_thread = Arc::clone(&promoted_preload);
    let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded::<DecodeCtrl>();
    let (prepare_tx, prepare_rx) = crossbeam_channel::unbounded::<DecodePrepareMsg>();

    let join = std::thread::Builder::new()
        .name("stellatune-decode".to_string())
        .spawn(move || {
            run_decode_worker(
                events,
                internal_tx,
                plugins,
                ctrl_rx,
                prepare_rx,
                Arc::clone(&runtime_state),
                Arc::clone(&promoted_preload_for_thread),
            )
        })
        .expect("failed to spawn stellatune-decode thread");

    DecodeWorker {
        ctrl_tx,
        prepare_tx,
        promoted_preload,
        join,
    }
}

fn run_decode_worker(
    events: Arc<EventHub>,
    internal_tx: Sender<InternalMsg>,
    plugins: Arc<Mutex<stellatune_plugins::PluginManager>>,
    ctrl_rx: Receiver<DecodeCtrl>,
    prepare_rx: Receiver<DecodePrepareMsg>,
    runtime_state: Arc<AtomicU8>,
    promoted_preload: Arc<Mutex<Option<PromotedPreload>>>,
) {
    while let Ok(msg) = prepare_rx.recv() {
        let prepare = match msg {
            DecodePrepareMsg::Prepare(prepare) => {
                set_decode_worker_state(
                    &runtime_state,
                    DecodeWorkerState::Prepared,
                    "prepare received",
                );
                prepare
            }
            DecodePrepareMsg::Shutdown => {
                set_decode_worker_state(&runtime_state, DecodeWorkerState::Idle, "shutdown");
                break;
            }
        };

        // Clear stale controls from the previous session before switching tracks.
        while ctrl_rx.try_recv().is_ok() {}

        let promoted =
            take_matching_promoted_preload(&promoted_preload, &prepare.path, prepare.start_at_ms);
        let (preopened, predecoded) = match promoted {
            Some(promoted) => (
                Some((promoted.decoder, promoted.track_info)),
                Some(promoted.chunk),
            ),
            None => (None, None),
        };

        let (setup_tx, setup_rx) = crossbeam_channel::bounded::<DecodeCtrl>(1);
        if setup_tx
            .send(DecodeCtrl::Setup {
                producer: Arc::clone(&prepare.producer),
                target_sample_rate: prepare.target_sample_rate,
                target_channels: prepare.target_channels,
                predecoded,
                start_at_ms: prepare.start_at_ms,
                output_enabled: Arc::clone(&prepare.output_enabled),
                buffer_prefill_cap_ms: prepare.buffer_prefill_cap_ms,
                lfe_mode: prepare.lfe_mode,
                output_sink_tx: None,
                output_sink_only: prepare.output_sink_only,
            })
            .is_err()
        {
            let _ = prepare
                .spec_tx
                .send(Err("failed to setup decode session".to_string()));
            set_decode_worker_state(&runtime_state, DecodeWorkerState::Idle, "setup failed");
            continue;
        }

        decode_thread(DecodeThreadArgs {
            path: prepare.path,
            events: Arc::clone(&events),
            internal_tx: internal_tx.clone(),
            plugins: Arc::clone(&plugins),
            preopened,
            ctrl_rx: ctrl_rx.clone(),
            setup_rx,
            spec_tx: prepare.spec_tx,
            runtime_state: Arc::clone(&runtime_state),
        });
        set_decode_worker_state(
            &runtime_state,
            DecodeWorkerState::Idle,
            "decode session completed",
        );
    }
}

fn set_decode_worker_state(runtime_state: &Arc<AtomicU8>, next: DecodeWorkerState, reason: &str) {
    let prev = runtime_state.swap(next as u8, Ordering::Relaxed);
    if prev == next as u8 {
        return;
    }
    let prev = DecodeWorkerState::from_u8(prev);
    debug!(from = ?prev, to = ?next, reason, "decode worker state");
}

fn take_matching_promoted_preload(
    promoted_preload: &Arc<Mutex<Option<PromotedPreload>>>,
    path: &str,
    start_at_ms: i64,
) -> Option<PromotedPreload> {
    let Ok(mut slot) = promoted_preload.lock() else {
        return None;
    };
    let Some(cached) = slot.take() else {
        debug_metrics::note_promote_lookup(debug_metrics::PromoteLookupResult::MissEmpty);
        return None;
    };
    let expected_ms = start_at_ms.max(0) as u64;
    if cached.path == path && cached.position_ms == expected_ms {
        debug_metrics::note_promote_lookup(debug_metrics::PromoteLookupResult::Hit);
        return Some(cached);
    }
    debug_metrics::note_promote_lookup(debug_metrics::PromoteLookupResult::MissMismatch);
    *slot = Some(cached);
    None
}

fn create_output_pipeline(
    backend: stellatune_output::AudioBackend,
    device_id: Option<String>,
    out_spec: OutputSpec,
    volume: Arc<AtomicU32>,
    internal_tx: Sender<InternalMsg>,
    device_output_enabled: bool,
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
    let transition_gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let transition_target_gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));

    let output = if device_output_enabled {
        let output_consumer = GatedConsumer {
            inner: Arc::clone(&consumer),
            enabled: Arc::clone(&output_enabled),
            volume,
            transition_gain: Arc::clone(&transition_gain),
            transition_target_gain: Arc::clone(&transition_target_gain),
            transition_current: 1.0,
            transition_step: (1.0
                / ((out_spec.sample_rate as f32 * SEEK_TRACK_FADE_RAMP_MS as f32) / 1000.0)
                    .max(1.0))
            .min(1.0),
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
        Some(output)
    } else {
        None
    };

    Ok(OutputPipeline {
        _output: output,
        producer,
        output_enabled,
        buffered_samples,
        underrun_callbacks,
        transition_gain,
        transition_target_gain,
        backend,
        device_id,
        device_output_enabled,
        out_sample_rate: out_spec.sample_rate,
        out_channels: out_spec.channels,
    })
}

pub(crate) struct StartSessionArgs<'a> {
    pub(crate) path: String,
    pub(crate) decode_worker: &'a DecodeWorker,
    pub(crate) internal_tx: Sender<InternalMsg>,
    pub(crate) backend: stellatune_output::AudioBackend,
    pub(crate) device_id: Option<String>,
    pub(crate) match_track_sample_rate: bool,
    pub(crate) gapless_playback: bool,
    pub(crate) out_spec: OutputSpec,
    pub(crate) start_at_ms: i64,
    pub(crate) volume: Arc<AtomicU32>,
    pub(crate) lfe_mode: stellatune_core::LfeMode,
    pub(crate) output_sink_only: bool,
    pub(crate) output_pipeline: &'a mut Option<OutputPipeline>,
}

pub(crate) fn start_session(args: StartSessionArgs<'_>) -> Result<PlaybackSession, String> {
    let StartSessionArgs {
        path,
        decode_worker,
        internal_tx,
        backend,
        device_id,
        match_track_sample_rate,
        gapless_playback,
        out_spec,
        start_at_ms,
        volume,
        lfe_mode,
        output_sink_only,
        output_pipeline,
    } = args;

    let t0 = Instant::now();
    let start_position_ms = start_at_ms.max(0) as u64;
    let promoted_track_info = decode_worker.peek_promoted_track_info(&path, start_position_ms);
    debug!(
        %path,
        start_at_ms,
        backend = ?backend,
        device_id = device_id.as_deref().unwrap_or("system-default"),
        match_track_sample_rate,
        gapless_playback,
        "start_session requested"
    );
    let (spec_tx, spec_rx) = crossbeam_channel::bounded::<Result<TrackDecodeInfo, String>>(1);

    let mut desired_out_spec = out_spec;
    if !output_sink_only
        && match_track_sample_rate
        && matches!(
            backend,
            stellatune_output::AudioBackend::WasapiExclusive
        )
        && let Some(info) = promoted_track_info.as_ref()
    {
        let candidate = OutputSpec {
            sample_rate: info.sample_rate,
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
                || p.device_output_enabled == output_sink_only
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
            !output_sink_only,
        )?);
    }

    let pipeline = output_pipeline
        .as_ref()
        .ok_or_else(|| "output pipeline not initialized".to_string())?;
    if let Ok(mut producer) = pipeline.producer.lock() {
        producer.clear();
    }

    decode_worker
        .prepare_tx
        .send(DecodePrepareMsg::Prepare(DecodePrepare {
            path,
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
            output_sink_only,
            spec_tx,
        }))
        .map_err(|_| "decode worker unavailable".to_string())?;

    let t_after_prepare = Instant::now();
    let track_info = match spec_rx.recv() {
        Ok(Ok(info)) => info,
        Ok(Err(message)) => return Err(message),
        Err(_) => return Err("decoder thread exited unexpectedly".to_string()),
    };
    let prepare_wait_ms = t_after_prepare.elapsed().as_millis() as u64;
    let session_total_ms = t0.elapsed().as_millis() as u64;
    debug!(
        prepare_wait_ms,
        session_total_ms,
        pipeline_rebuilt = rebuild_pipeline,
        out_sample_rate = pipeline.out_sample_rate,
        out_channels = pipeline.out_channels,
        track_sample_rate = track_info.sample_rate,
        track_channels = track_info.channels,
        "start_session ready"
    );
    debug_metrics::note_session_start(prepare_wait_ms, session_total_ms, rebuild_pipeline);
    Ok(PlaybackSession {
        ctrl_tx: decode_worker.ctrl_tx.clone(),
        output_enabled: Arc::clone(&pipeline.output_enabled),
        buffered_samples: Arc::clone(&pipeline.buffered_samples),
        underrun_callbacks: Arc::clone(&pipeline.underrun_callbacks),
        transition_gain: Arc::clone(&pipeline.transition_gain),
        transition_target_gain: Arc::clone(&pipeline.transition_target_gain),
        out_sample_rate: pipeline.out_sample_rate,
        out_channels: pipeline.out_channels,
        track_info,
    })
}

struct GatedConsumer {
    inner: Arc<Mutex<RingBufferConsumer<f32>>>,
    enabled: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    transition_gain: Arc<AtomicU32>,
    transition_target_gain: Arc<AtomicU32>,
    transition_current: f32,
    transition_step: f32,
    buffered_samples: Arc<AtomicUsize>,
    underrun_callbacks: Arc<AtomicU64>,
    scratch: Vec<f32>,
    scratch_len: usize,
    scratch_cursor: usize,
}

impl GatedConsumer {
    fn next_transition_gain(&mut self) -> f32 {
        let target =
            f32::from_bits(self.transition_target_gain.load(Ordering::Relaxed)).clamp(0.0, 1.0);
        if self.transition_current < target {
            self.transition_current = (self.transition_current + self.transition_step).min(target);
        } else if self.transition_current > target {
            self.transition_current = (self.transition_current - self.transition_step).max(target);
        }
        self.transition_gain
            .store(self.transition_current.to_bits(), Ordering::Relaxed);
        self.transition_current
    }
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
        let transition = self.next_transition_gain();
        Some(sample * v * transition)
    }

    fn on_output(&mut self, requested: usize, provided: usize) {
        let buffered = self.inner.lock().map(|inner| inner.len()).unwrap_or(0);
        self.buffered_samples.store(buffered, Ordering::Relaxed);
        if self.enabled.load(Ordering::Relaxed) && provided < requested {
            self.underrun_callbacks.fetch_add(1, Ordering::Relaxed);
        }
    }
}
