use std::time::Duration;

use crate::config::gain::GainTransitionConfig;
use crate::config::sink::{SinkLatencyConfig, SinkRecoveryConfig};

/// High-level playback state reported by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// Playback is stopped and no stream is advancing.
    Stopped,
    /// Playback is paused with an active track session.
    Paused,
    /// Playback is actively producing output.
    Playing,
}

/// Pause strategy requested by control operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseBehavior {
    /// Pause immediately without draining buffered sink data.
    Immediate,
    /// Pause after draining buffered sink data.
    DrainSink,
}

/// Stop strategy requested by control operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBehavior {
    /// Stop immediately without draining buffered sink data.
    Immediate,
    /// Stop after draining buffered sink data.
    DrainSink,
}

/// Low-frequency effects routing mode for the mixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfeMode {
    /// Mute LFE content.
    #[default]
    Mute,
    /// Fold LFE content into front channels.
    MixToFront,
}

/// Resampler quality preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResampleQuality {
    /// Lowest CPU cost with lower quality.
    Fast,
    /// Balanced quality and CPU profile.
    Balanced,
    /// High quality preset.
    #[default]
    High,
    /// Highest quality preset with higher CPU cost.
    Ultra,
}

/// Event payload emitted by the engine event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Player state changed.
    StateChanged {
        /// The new player state.
        state: PlayerState,
    },
    /// Active track token changed.
    TrackChanged {
        /// The new active track token.
        track_token: String,
    },
    /// Sink recovery retry was scheduled.
    Recovering {
        /// Retry attempt index (1-based).
        attempt: u32,
        /// Backoff delay before the next retry, in milliseconds.
        backoff_ms: u64,
    },
    /// Playback position update in milliseconds.
    Position {
        /// Position in milliseconds.
        position_ms: i64,
    },
    /// Volume update notification.
    VolumeChanged {
        /// Target volume level.
        volume: f32,
        /// Monotonic sequence number supplied by the caller.
        seq: u64,
    },
    /// Active track reached end-of-stream.
    Eof,
    /// Audio playback has physically started producing output for the current track.
    AudioStart,
    /// Audio playback has physically finished for the current track.
    AudioEnd,
    /// Error message emitted by the runtime boundary.
    Error {
        /// Human-readable error text.
        message: String,
    },
}

/// Snapshot of current engine runtime state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSnapshot {
    /// Current player state.
    pub state: PlayerState,
    /// Active track token, if one exists.
    pub current_track: Option<String>,
    /// Current playback position in milliseconds.
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

/// Runtime configuration for engine control and worker behavior.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Timeout for control actor command round-trips.
    pub command_timeout: Duration,
    /// Timeout used for decode-worker command calls.
    pub decode_command_timeout: Duration,
    /// Sleep interval while playing with pending sink blocks.
    pub decode_playing_pending_block_sleep: Duration,
    /// Sleep interval while playing and idle.
    pub decode_playing_idle_sleep: Duration,
    /// Sleep interval while globally idle.
    pub decode_idle_sleep: Duration,
    /// Timeout for sink control commands.
    pub sink_control_timeout: Duration,
    /// Sink queue/latency policy.
    pub sink_latency: SinkLatencyConfig,
    /// Sink recovery retry policy.
    pub sink_recovery: SinkRecoveryConfig,
    /// Gain transition policy.
    pub gain_transition: GainTransitionConfig,
    /// Decode worker command channel capacity.
    pub decode_command_capacity: usize,
    /// Event hub broadcast capacity.
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
