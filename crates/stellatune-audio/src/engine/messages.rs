use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use stellatune_output::OutputSpec;
use stellatune_plugins::DspInstance;

use crate::ring_buffer::RingBufferProducer;

pub(crate) enum DecodeCtrl {
    Setup {
        producer: RingBufferProducer<f32>,
        target_sample_rate: u32,
        target_channels: u16,
        start_at_ms: i64,
        output_enabled: Arc<AtomicBool>,
    },
    SetDspChain {
        chain: Vec<DspInstance>,
    },
    Play,
    Pause,
    SeekMs {
        position_ms: i64,
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
}

pub(crate) enum InternalMsg {
    Eof,
    Error(String),
    OutputError(String),
    Position(i64),
    OutputSpecReady { spec: OutputSpec, took_ms: u64 },
    OutputSpecFailed { message: String, took_ms: u64 },
}
