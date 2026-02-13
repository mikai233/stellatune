use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crossbeam_channel::{Sender, TrySendError};
use stellatune_core::TrackDecodeInfo;
use stellatune_output::OutputSpec;

use crate::ring_buffer::RingBufferProducer;

pub(crate) struct PredecodedChunk {
    pub(crate) samples: Vec<f32>,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u16,
    pub(crate) start_at_ms: u64,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodeWorkerState {
    Idle = 0,
    Prepared = 1,
    Playing = 2,
    Paused = 3,
}

impl DecodeWorkerState {
    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Prepared,
            2 => Self::Playing,
            3 => Self::Paused,
            _ => Self::Idle,
        }
    }
}

pub(crate) enum OutputSinkWrite {
    Samples(Vec<f32>),
    Shutdown { drain: bool },
}

#[derive(Clone)]
pub(crate) struct OutputSinkTx {
    tx: Sender<OutputSinkWrite>,
    pending_samples: Arc<AtomicUsize>,
}

impl OutputSinkTx {
    pub(crate) fn new(tx: Sender<OutputSinkWrite>, pending_samples: Arc<AtomicUsize>) -> Self {
        Self {
            tx,
            pending_samples,
        }
    }

    pub(crate) fn try_send_samples(
        &self,
        samples: Vec<f32>,
    ) -> Result<(), TrySendError<OutputSinkWrite>> {
        let count = samples.len();
        self.pending_samples.fetch_add(count, Ordering::Relaxed);
        match self.tx.try_send(OutputSinkWrite::Samples(samples)) {
            Ok(()) => Ok(()),
            Err(e) => {
                if count > 0 {
                    let _ = self.pending_samples.fetch_update(
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                        |current| Some(current.saturating_sub(count)),
                    );
                }
                Err(e)
            }
        }
    }
}

pub(crate) enum DecodeCtrl {
    Setup {
        producer: Arc<Mutex<RingBufferProducer<f32>>>,
        target_sample_rate: u32,
        target_channels: u16,
        predecoded: Option<PredecodedChunk>,
        start_at_ms: i64,
        output_enabled: Arc<AtomicBool>,
        buffer_prefill_cap_ms: i64,
        lfe_mode: stellatune_core::LfeMode,
        output_sink_tx: Option<OutputSinkTx>,
        output_sink_chunk_frames: u32,
        output_sink_only: bool,
    },
    SetDspChain {
        chain: Vec<RuntimeDspChainEntry>,
    },
    Play,
    Pause,
    SeekMs {
        position_ms: i64,
    },
    SetLfeMode {
        mode: stellatune_core::LfeMode,
    },
    SetOutputSinkTx {
        tx: Option<OutputSinkTx>,
        output_sink_chunk_frames: u32,
    },
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeDspChainEntry {
    pub(crate) plugin_id: String,
    pub(crate) type_id: String,
    pub(crate) config_json: String,
}

pub(crate) enum EngineCtrl {
    SetDspChain {
        chain: Vec<stellatune_core::DspChainItem>,
    },
    SourceListItemsJson {
        plugin_id: String,
        type_id: String,
        config_json: String,
        request_json: String,
        resp_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
    },
    LyricsSearchJson {
        plugin_id: String,
        type_id: String,
        query_json: String,
        resp_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
    },
    LyricsFetchJson {
        plugin_id: String,
        type_id: String,
        track_json: String,
        resp_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
    },
    OutputSinkListTargetsJson {
        plugin_id: String,
        type_id: String,
        config_json: String,
        resp_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
    },
    QuiescePluginUsage {
        plugin_id: String,
        resp_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    ReloadPlugins {
        dir: String,
    },
    SetLfeMode {
        mode: stellatune_core::LfeMode,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PluginReloadSummary {
    pub(crate) dir: String,
    pub(crate) prev_count: usize,
    pub(crate) loaded_ids: Vec<String>,
    pub(crate) loaded_count: usize,
    pub(crate) deactivated_count: usize,
    pub(crate) unloaded_generations: usize,
    pub(crate) load_errors: Vec<String>,
    pub(crate) fatal_error: Option<String>,
}

pub(crate) enum InternalMsg {
    Eof,
    Error(String),
    OutputError(String),
    Position {
        path: String,
        ms: i64,
    },
    OutputSpecReady {
        spec: OutputSpec,
        took_ms: u64,
        token: u64,
    },
    OutputSpecFailed {
        message: String,
        took_ms: u64,
        token: u64,
    },
    PreloadReady {
        path: String,
        position_ms: u64,
        track_info: TrackDecodeInfo,
        chunk: PredecodedChunk,
        took_ms: u64,
        token: u64,
    },
    PreloadFailed {
        path: String,
        position_ms: u64,
        message: String,
        took_ms: u64,
        token: u64,
    },
}
