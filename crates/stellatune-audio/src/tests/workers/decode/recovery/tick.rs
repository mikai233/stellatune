use std::sync::Arc;
use std::time::{Duration, Instant};

use stellatune_audio_core::pipeline::context::InputRef;

use crate::config::engine::EngineConfig;
use crate::config::sink::SinkRecoveryConfig;
use crate::pipeline::assembly::PipelineAssembler;
use crate::workers::decode::DecodeWorkerEvent;

use super::harness::{
    FailingAssembler, FailingRuntime, SuccessAssembler, SuccessRuntime, event_callback,
    shutdown_state, test_config, test_state,
};

#[test]
fn try_sink_recovery_tick_returns_true_when_retry_is_not_due() {
    let config = test_config();
    let mut state = test_state(&config);
    state.active_input = Some(InputRef::TrackToken("track-a".to_string()));
    state.recovery_retry_at = Some(Instant::now() + Duration::from_secs(1));
    let (callback, events) = event_callback();
    let assembler: Arc<dyn PipelineAssembler> = Arc::new(FailingAssembler);
    let mut runtime = FailingRuntime;

    let keep_running = crate::workers::decode::recovery::try_sink_recovery_tick(
        &assembler,
        &callback,
        &mut runtime,
        &mut state,
        &config,
    );

    assert!(keep_running);
    assert_eq!(state.recovery_attempts, 0);
    assert!(state.recovery_retry_at.is_some());
    assert!(events.lock().expect("events mutex poisoned").is_empty());
}

#[test]
fn try_sink_recovery_tick_failure_reschedules_and_emits_recovering() {
    let config = test_config();
    let mut state = test_state(&config);
    state.active_input = Some(InputRef::TrackToken("track-a".to_string()));
    state.recovery_retry_at = Some(Instant::now() - Duration::from_millis(1));
    let (callback, events) = event_callback();
    let assembler: Arc<dyn PipelineAssembler> = Arc::new(FailingAssembler);
    let mut runtime = FailingRuntime;

    let keep_running = crate::workers::decode::recovery::try_sink_recovery_tick(
        &assembler,
        &callback,
        &mut runtime,
        &mut state,
        &config,
    );

    assert!(keep_running);
    assert_eq!(state.recovery_attempts, 1);
    assert!(state.recovery_retry_at.is_some());
    let recorded = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(recorded.len(), 1);
    match &recorded[0] {
        DecodeWorkerEvent::Recovering {
            attempt,
            backoff_ms,
        } => {
            assert_eq!(*attempt, 2);
            assert_eq!(*backoff_ms, 40);
        },
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn try_sink_recovery_tick_exhaustion_emits_error_and_stops_retries() {
    let config = EngineConfig {
        sink_recovery: SinkRecoveryConfig {
            max_attempts: 1,
            initial_backoff: Duration::from_millis(20),
            max_backoff: Duration::from_millis(80),
        },
        ..EngineConfig::default()
    };
    let mut state = test_state(&config);
    state.active_input = Some(InputRef::TrackToken("track-a".to_string()));
    state.recovery_retry_at = Some(Instant::now() - Duration::from_millis(1));
    let (callback, events) = event_callback();
    let assembler: Arc<dyn PipelineAssembler> = Arc::new(FailingAssembler);
    let mut runtime = FailingRuntime;

    let keep_running = crate::workers::decode::recovery::try_sink_recovery_tick(
        &assembler,
        &callback,
        &mut runtime,
        &mut state,
        &config,
    );

    assert!(!keep_running);
    assert_eq!(state.recovery_attempts, 1);
    assert!(state.recovery_retry_at.is_none());
    let recorded = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(recorded.len(), 1);
    match &recorded[0] {
        DecodeWorkerEvent::Error(message) => assert!(message.contains("plan failed")),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn try_sink_recovery_tick_success_clears_retry_state_and_rebuilds_runner() {
    let config = test_config();
    let mut state = test_state(&config);
    state.active_input = Some(InputRef::TrackToken("track-a".to_string()));
    state.recovery_attempts = 2;
    state.recovery_retry_at = Some(Instant::now() - Duration::from_millis(1));
    let (callback, events) = event_callback();
    let assembler: Arc<dyn PipelineAssembler> = Arc::new(SuccessAssembler);
    let mut runtime = SuccessRuntime;

    let keep_running = crate::workers::decode::recovery::try_sink_recovery_tick(
        &assembler,
        &callback,
        &mut runtime,
        &mut state,
        &config,
    );

    assert!(keep_running);
    assert!(state.runner.is_some());
    assert_eq!(state.recovery_attempts, 0);
    assert!(state.recovery_retry_at.is_none());
    assert!(events.lock().expect("events mutex poisoned").is_empty());
    shutdown_state(state);
}
