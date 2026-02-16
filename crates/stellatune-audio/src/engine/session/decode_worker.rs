use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use tracing::debug;

use crate::engine::control::InternalDispatch;
use crate::types::{LfeMode, ResampleQuality, TrackDecodeInfo};

use crate::engine::decode::{DecodeThreadArgs, decode_thread};
use crate::engine::messages::{DecodeCtrl, DecodeWorkerState, PredecodedChunk};
use crate::ring_buffer::RingBufferProducer;

use super::debug_metrics;
pub(crate) struct PromotedPreload {
    pub(crate) path: String,
    pub(crate) position_ms: u64,
    pub(crate) track_info: TrackDecodeInfo,
    pub(crate) chunk: PredecodedChunk,
}

pub(super) struct DecodePrepare {
    pub(super) path: String,
    pub(super) producer: Arc<Mutex<RingBufferProducer<f32>>>,
    pub(super) target_sample_rate: u32,
    pub(super) target_channels: u16,
    pub(super) start_at_ms: i64,
    pub(super) output_enabled: Arc<AtomicBool>,
    pub(super) buffer_prefill_cap_ms: i64,
    pub(super) lfe_mode: LfeMode,
    pub(super) output_sink_chunk_frames: u32,
    pub(super) output_sink_only: bool,
    pub(super) resample_quality: ResampleQuality,
    pub(super) spec_tx: Sender<Result<TrackDecodeInfo, String>>,
}

pub(super) enum DecodePrepareMsg {
    Prepare(DecodePrepare),
    Shutdown,
}

pub(crate) struct DecodeWorker {
    pub(crate) ctrl_tx: Sender<DecodeCtrl>,
    pub(super) prepare_tx: Sender<DecodePrepareMsg>,
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

    pub(crate) fn clear_promoted_preload(&self) {
        if let Ok(mut slot) = self.promoted_preload.lock() {
            *slot = None;
        }
    }

    pub(crate) fn shutdown(self) {
        let _ = self.ctrl_tx.send(DecodeCtrl::Stop);
        let _ = self.prepare_tx.send(DecodePrepareMsg::Shutdown);
        let _ = self.join.join();
    }
}

pub(crate) fn start_decode_worker(internal_tx: Sender<InternalDispatch>) -> DecodeWorker {
    debug_metrics::note_worker_start();
    let runtime_state = Arc::new(AtomicU8::new(DecodeWorkerState::Idle as u8));
    let promoted_preload = Arc::new(Mutex::new(None));
    let promoted_preload_for_thread = Arc::clone(&promoted_preload);
    let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded::<DecodeCtrl>();
    let (prepare_tx, prepare_rx) = crossbeam_channel::unbounded::<DecodePrepareMsg>();

    let join = std::thread::Builder::new()
        .name("stellatune-audio-decode".to_string())
        .spawn(move || {
            let _rt_guard = crate::output::enable_realtime_audio_thread();
            run_decode_worker(
                internal_tx,
                ctrl_rx,
                prepare_rx,
                Arc::clone(&runtime_state),
                Arc::clone(&promoted_preload_for_thread),
            )
        })
        .expect("failed to spawn stellatune-audio-decode thread");

    DecodeWorker {
        ctrl_tx,
        prepare_tx,
        promoted_preload,
        join,
    }
}

fn run_decode_worker(
    internal_tx: Sender<InternalDispatch>,
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
            },
            DecodePrepareMsg::Shutdown => {
                set_decode_worker_state(&runtime_state, DecodeWorkerState::Idle, "shutdown");
                break;
            },
        };

        // Clear stale controls from the previous session before switching tracks.
        while ctrl_rx.try_recv().is_ok() {}

        let promoted =
            take_matching_promoted_preload(&promoted_preload, &prepare.path, prepare.start_at_ms);
        let predecoded = match promoted {
            Some(promoted) => Some(promoted.chunk),
            None => None,
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
                output_sink_chunk_frames: prepare.output_sink_chunk_frames,
                output_sink_only: prepare.output_sink_only,
                resample_quality: prepare.resample_quality,
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
            internal_tx: internal_tx.clone(),
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
