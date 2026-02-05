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
        buffer_prefill_cap_ms: i64,
        lfe_mode: stellatune_core::LfeMode,
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
}
