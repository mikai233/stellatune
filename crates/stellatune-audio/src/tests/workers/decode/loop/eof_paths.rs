use std::time::Duration;

use super::harness::{EnsureAction, LoopHarness, RuntimeState};

#[test]
fn eof_promotes_prewarmed_next_without_rebuilding_pipeline() {
    let mut runtime_state = RuntimeState::default();
    runtime_state.set_track_blocks("track-a", 3);
    runtime_state.set_track_blocks("track-b", 3);
    let harness = LoopHarness::start(runtime_state);

    harness
        .open("track-a", false)
        .expect("open track-a in paused mode should succeed");
    harness
        .queue_next("track-b")
        .expect("queue_next should prewarm track-b successfully");
    harness.play().expect("play should succeed");
    harness
        .wait_for_track_changed("track-b", Duration::from_secs(2))
        .expect("track-b should be promoted from prewarmed runner");

    assert_eq!(
        harness.ensure_count("track-b"),
        1,
        "prewarmed promotion should not rebuild track-b on EOF",
    );

    harness.shutdown();
}

#[test]
fn eof_falls_back_to_queued_next_open_when_prewarm_failed() {
    let mut runtime_state = RuntimeState::default();
    runtime_state.set_track_blocks("track-a", 3);
    runtime_state.set_track_blocks("track-b", 3);
    runtime_state.set_ensure_script(
        "track-b",
        vec![EnsureAction::Fail("prewarm failed"), EnsureAction::Succeed],
    );
    let harness = LoopHarness::start(runtime_state);

    harness
        .open("track-a", false)
        .expect("open track-a in paused mode should succeed");
    let queue_result = harness.queue_next("track-b");
    assert!(
        queue_result.is_err(),
        "queue_next should report prewarm failure in this test"
    );
    let queue_error = queue_result.expect_err("queue_next should fail");
    assert!(
        queue_error.to_string().contains("prewarm failed"),
        "queue_next error should contain prewarm cause"
    );

    harness.play().expect("play should succeed");
    harness
        .wait_for_track_changed("track-b", Duration::from_secs(2))
        .expect("queued-next fallback should open track-b on EOF");

    assert_eq!(
        harness.ensure_count("track-b"),
        2,
        "fallback path should rebuild track-b after failed prewarm",
    );

    harness.shutdown();
}
