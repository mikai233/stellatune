use std::time::Instant;

use stellatune_audio_core::pipeline::context::InputRef;

use crate::workers::decode::DecodeWorkerEvent;

use super::harness::{event_callback, test_config, test_state};

#[test]
fn schedule_sink_recovery_skips_when_no_active_input() {
    let config = test_config();
    let mut state = test_state(&config);
    let (callback, events) = event_callback();

    let scheduled = crate::workers::decode::recovery::schedule_sink_recovery(&callback, &mut state);

    assert!(!scheduled);
    assert!(state.recovery_retry_at.is_none());
    assert_eq!(state.recovery_attempts, 0);
    assert!(events.lock().expect("events mutex poisoned").is_empty());
}

#[test]
fn schedule_sink_recovery_emits_first_attempt_and_sets_retry_deadline() {
    let config = test_config();
    let mut state = test_state(&config);
    state.active_input = Some(InputRef::TrackToken("track-a".to_string()));
    state.recovery_attempts = 2;
    let (callback, events) = event_callback();
    let before = Instant::now();

    let scheduled = crate::workers::decode::recovery::schedule_sink_recovery(&callback, &mut state);

    assert!(scheduled);
    assert_eq!(state.recovery_attempts, 0);
    let retry_at = state
        .recovery_retry_at
        .expect("recovery retry deadline should be set");
    assert!(retry_at >= before + config.sink_recovery.initial_backoff);
    let recorded = events.lock().expect("events mutex poisoned").clone();
    assert_eq!(recorded.len(), 1);
    match &recorded[0] {
        DecodeWorkerEvent::Recovering {
            attempt,
            backoff_ms,
        } => {
            assert_eq!(*attempt, 1);
            assert_eq!(
                *backoff_ms,
                config.sink_recovery.initial_backoff.as_millis() as u64
            );
        },
        other => panic!("unexpected event: {other:?}"),
    }
}
