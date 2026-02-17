use std::time::Duration;

use crate::config::gain::GainTransitionConfig;
use crate::config::sink::{SinkLatencyConfig, SinkRecoveryConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Paused,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseBehavior {
    Immediate,
    DrainSink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBehavior {
    Immediate,
    DrainSink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfeMode {
    #[default]
    Mute,
    MixToFront,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResampleQuality {
    Fast,
    Balanced,
    #[default]
    High,
    Ultra,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    StateChanged { state: PlayerState },
    TrackChanged { track_token: String },
    Recovering { attempt: u32, backoff_ms: u64 },
    Position { position_ms: i64 },
    VolumeChanged { volume: f32, seq: u64 },
    Eof,
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSnapshot {
    pub state: PlayerState,
    pub current_track: Option<String>,
    pub position_ms: i64,
}

impl Default for EngineSnapshot {
    fn default() -> Self {
        Self {
            state: PlayerState::Stopped,
            current_track: None,
            position_ms: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub command_timeout: Duration,
    pub decode_command_timeout: Duration,
    pub decode_playing_pending_block_sleep: Duration,
    pub decode_playing_idle_sleep: Duration,
    pub decode_idle_sleep: Duration,
    pub sink_control_timeout: Duration,
    pub sink_latency: SinkLatencyConfig,
    pub sink_recovery: SinkRecoveryConfig,
    pub gain_transition: GainTransitionConfig,
    pub decode_command_capacity: usize,
    pub event_capacity: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            command_timeout: Duration::from_secs(12),
            decode_command_timeout: Duration::from_secs(12),
            decode_playing_pending_block_sleep: Duration::from_micros(250),
            decode_playing_idle_sleep: Duration::from_millis(2),
            decode_idle_sleep: Duration::from_millis(8),
            sink_control_timeout: Duration::from_millis(500),
            sink_latency: SinkLatencyConfig::default(),
            sink_recovery: SinkRecoveryConfig::default(),
            gain_transition: GainTransitionConfig::default(),
            decode_command_capacity: 128,
            event_capacity: 256,
        }
    }
}
