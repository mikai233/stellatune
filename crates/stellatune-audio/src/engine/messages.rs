use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use crossbeam_channel::Sender;
use stellatune_core::TrackDecodeInfo;
use stellatune_output::OutputSpec;
use stellatune_plugins::DspInstance;

use crate::engine::decode::decoder::EngineDecoder;
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
    Shutdown,
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
        output_sink_tx: Option<Sender<OutputSinkWrite>>,
        output_sink_only: bool,
    },
    SetDspChain {
        chain: Vec<DspInstance>,
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
        tx: Option<Sender<OutputSinkWrite>>,
    },
    Stop,
}

pub(crate) enum EngineCtrl {
    SetDspChain {
        chain: Vec<stellatune_core::DspChainItem>,
    },
    ReloadPlugins {
        dir: String,
    },
    ReloadPluginsWithDisabled {
        dir: String,
        disabled_ids: Vec<String>,
    },
    SetLfeMode {
        mode: stellatune_core::LfeMode,
    },
}

pub(crate) enum InternalMsg {
    Eof,
    Error(String),
    OutputError(String),
    Position(i64),
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
        decoder: EngineDecoder,
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
