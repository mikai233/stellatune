use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crossbeam_channel::Sender;
use tracing::debug;

use crate::engine::control::InternalDispatchTx;
use crate::output::{OutputHandle, OutputSpec};
use crate::types::{LfeMode, ResampleQuality, TrackDecodeInfo};

use crate::engine::config::{BUFFER_PREFILL_CAP_MS, BUFFER_PREFILL_CAP_MS_EXCLUSIVE};
use crate::engine::messages::DecodeCtrl;
use crate::ring_buffer::RingBufferProducer;

pub(crate) mod decode_worker;
pub(crate) mod output_sink_worker;
mod pipeline;

use decode_worker::{DecodePrepare, DecodePrepareMsg, DecodeWorker};
use pipeline::create_output_pipeline;

const OUTPUT_CONSUMER_CHUNK_SAMPLES: usize = 1024;
pub(crate) const OUTPUT_SINK_QUEUE_CAP_MESSAGES: usize = 16;
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
            },
            PromoteLookupResult::MissEmpty => {
                PROMOTE_MISS_EMPTY.fetch_add(1, Ordering::Relaxed);
            },
            PromoteLookupResult::MissMismatch => {
                PROMOTE_MISS_MISMATCH.fetch_add(1, Ordering::Relaxed);
            },
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
    pub(crate) transition_ramp_ms: Arc<AtomicU32>,
    pub(crate) backend: crate::output::AudioBackend,
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
    pub(crate) transition_ramp_ms: Arc<AtomicU32>,
    pub(crate) out_sample_rate: u32,
    pub(crate) out_channels: u16,
    pub(crate) track_info: TrackDecodeInfo,
}

pub(crate) struct StartSessionArgs<'a> {
    pub(crate) path: String,
    pub(crate) decode_worker: &'a DecodeWorker,
    pub(crate) internal_tx: InternalDispatchTx,
    pub(crate) backend: crate::output::AudioBackend,
    pub(crate) device_id: Option<String>,
    pub(crate) match_track_sample_rate: bool,
    pub(crate) gapless_playback: bool,
    pub(crate) out_spec: OutputSpec,
    pub(crate) start_at_ms: i64,
    pub(crate) volume: Arc<AtomicU32>,
    pub(crate) lfe_mode: LfeMode,
    pub(crate) output_sink_chunk_frames: u32,
    pub(crate) output_sink_only: bool,
    pub(crate) resample_quality: ResampleQuality,
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
        output_sink_chunk_frames,
        output_sink_only,
        resample_quality,
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
        && matches!(backend, crate::output::AudioBackend::WasapiExclusive)
        && let Some(info) = promoted_track_info.as_ref()
    {
        let candidate = OutputSpec {
            sample_rate: info.sample_rate,
            channels: out_spec.channels,
        };
        if crate::output::supports_output_spec(backend, device_id.clone(), candidate) {
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
            && if output_sink_only {
                existing_spec.sample_rate == desired_out_spec.sample_rate
                    && existing_spec.channels == desired_out_spec.channels
            } else if gapless_playback {
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
    // Session switch can reuse the same output pipeline. Reset runtime-facing state so
    // control tick never observes stale buffered samples and accidentally opens the gate early.
    pipeline.output_enabled.store(false, Ordering::Release);
    pipeline.buffered_samples.store(0, Ordering::Relaxed);

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
                crate::output::AudioBackend::WasapiExclusive => BUFFER_PREFILL_CAP_MS_EXCLUSIVE,
                _ => BUFFER_PREFILL_CAP_MS,
            },
            lfe_mode,
            output_sink_chunk_frames,
            output_sink_only,
            resample_quality,
            spec_tx,
        }))
        .map_err(|_| {
            "decode worker unavailable: prepare channel disconnected (worker thread exited)"
                .to_string()
        })?;

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
        transition_ramp_ms: Arc::clone(&pipeline.transition_ramp_ms),
        out_sample_rate: pipeline.out_sample_rate,
        out_channels: pipeline.out_channels,
        track_info,
    })
}
