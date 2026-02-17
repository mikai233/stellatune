use crossbeam_channel::Sender;

use crate::config::engine::{PlayerState, StopBehavior};
use crate::pipeline::assembly::PipelineRuntime;
use crate::workers::decode::handlers::gain_transition;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::util::update_state;
use crate::workers::decode::{DecodeWorkerEvent, DecodeWorkerEventCallback};

pub(crate) fn handle(
    behavior: StopBehavior,
    resp_tx: Sender<Result<(), String>>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    let transition = state.gain_transition;
    let mut stop_error: Option<String> = None;
    if let Some(active_runner) = state.runner.as_mut() {
        if state.state == PlayerState::Playing {
            let available_frames_hint = active_runner.playable_remaining_frames_hint();
            let _ = gain_transition::run_interrupt_fade_out(
                active_runner,
                &mut state.sink_session,
                &mut state.ctx,
                transition,
                transition.stop_fade_out_ms,
                available_frames_hint,
            );
        }
        if let Err(error) =
            active_runner.stop_with_behavior(behavior, &mut state.sink_session, &mut state.ctx)
        {
            stop_error = Some(error.to_string());
        }
    } else {
        state.sink_session.shutdown(false);
    }
    pipeline_runtime.reset();
    state.runner = None;
    state.reset_context();
    state.active_input = None;
    state.queued_next_input = None;
    state.prewarmed_next = None;
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;
    update_state(callback, &mut state.state, PlayerState::Stopped);
    callback(DecodeWorkerEvent::Position { position_ms: 0 });
    let _ = resp_tx.send(match stop_error {
        Some(error) => Err(error),
        None => Ok(()),
    });
    false
}
