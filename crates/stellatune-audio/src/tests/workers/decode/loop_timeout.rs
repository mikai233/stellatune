use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::engine::{EngineConfig, PlayerState};
use crate::pipeline::runtime::dsp::control::MasterGainHotControl;
use crate::workers::decode::state::DecodeWorkerState;

fn test_state(config: &EngineConfig) -> DecodeWorkerState {
    DecodeWorkerState::new(
        config.sink_latency,
        config.sink_recovery,
        config.gain_transition,
        config.sink_control_timeout,
        Arc::new(MasterGainHotControl::default()),
    )
}

#[test]
fn compute_loop_timeout_uses_idle_sleep_when_not_playing() {
    let config = EngineConfig {
        decode_idle_sleep: Duration::from_millis(17),
        ..EngineConfig::default()
    };
    let mut state = test_state(&config);
    state.state = PlayerState::Stopped;

    let timeout = crate::workers::decode::worker_loop::compute_loop_timeout(&state, &config);

    assert_eq!(timeout, config.decode_idle_sleep);
}

#[test]
fn compute_loop_timeout_caps_recovery_wait_to_pending_block_sleep() {
    let config = EngineConfig {
        decode_playing_pending_block_sleep: Duration::from_millis(6),
        ..EngineConfig::default()
    };
    let mut state = test_state(&config);
    state.state = PlayerState::Playing;
    state.recovery_retry_at = Some(Instant::now() + Duration::from_millis(300));

    let timeout = crate::workers::decode::worker_loop::compute_loop_timeout(&state, &config);

    assert_eq!(timeout, config.decode_playing_pending_block_sleep);
}

#[test]
fn compute_loop_timeout_uses_playing_idle_sleep_without_recovery() {
    let config = EngineConfig {
        decode_playing_idle_sleep: Duration::from_millis(3),
        ..EngineConfig::default()
    };
    let mut state = test_state(&config);
    state.state = PlayerState::Playing;
    state.recovery_retry_at = None;

    let timeout = crate::workers::decode::worker_loop::compute_loop_timeout(&state, &config);

    assert_eq!(timeout, config.decode_playing_idle_sleep);
}
