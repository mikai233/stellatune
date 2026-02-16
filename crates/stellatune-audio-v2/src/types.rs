use std::time::Duration;

use serde::{Deserialize, Serialize};
use stellatune_audio_core::pipeline::context::{TransitionCurve, TransitionTimePolicy};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DspChainStage {
    PreMix,
    PostMix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DspChainItem {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
    pub stage: DspChainStage,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DspChainSpec {
    pub items: Vec<DspChainItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    StateChanged { state: PlayerState },
    TrackChanged { track_token: String },
    Recovering { attempt: u32, backoff_ms: u64 },
    Position { position_ms: i64 },
    Eof,
    Error { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkLatencyConfig {
    pub target_latency_ms: u32,
    pub block_frames: u32,
    pub min_queue_blocks: usize,
    pub max_queue_blocks: usize,
}

impl SinkLatencyConfig {
    pub fn queue_capacity(self, sample_rate: u32) -> usize {
        let min_blocks = self.min_queue_blocks.max(1);
        let max_blocks = self.max_queue_blocks.max(min_blocks);
        let block_frames = self.block_frames.max(1) as u64;
        let target_frames = (sample_rate as u64 * self.target_latency_ms as u64).div_ceil(1000);
        let mut blocks = target_frames.div_ceil(block_frames) as usize;
        if blocks == 0 {
            blocks = 1;
        }
        blocks.clamp(min_blocks, max_blocks)
    }
}

impl Default for SinkLatencyConfig {
    fn default() -> Self {
        Self {
            target_latency_ms: 80,
            block_frames: 1024,
            min_queue_blocks: 2,
            max_queue_blocks: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkRecoveryConfig {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for SinkRecoveryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 6,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(2),
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainTransitionConfig {
    pub open_fade_in_ms: u32,
    pub play_fade_in_ms: u32,
    pub seek_fade_out_ms: u32,
    pub seek_fade_in_ms: u32,
    pub pause_fade_out_ms: u32,
    pub stop_fade_out_ms: u32,
    pub switch_fade_out_ms: u32,
    pub curve: TransitionCurve,
    pub fade_in_time_policy: TransitionTimePolicy,
    pub fade_out_time_policy: TransitionTimePolicy,
    pub interrupt_max_extra_wait_ms: u32,
}

impl Default for GainTransitionConfig {
    fn default() -> Self {
        Self {
            open_fade_in_ms: 24,
            play_fade_in_ms: 24,
            seek_fade_out_ms: 24,
            seek_fade_in_ms: 24,
            pause_fade_out_ms: 36,
            stop_fade_out_ms: 48,
            switch_fade_out_ms: 36,
            curve: TransitionCurve::EqualPower,
            fade_in_time_policy: TransitionTimePolicy::Exact,
            fade_out_time_policy: TransitionTimePolicy::FitToAvailable,
            interrupt_max_extra_wait_ms: 80,
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
