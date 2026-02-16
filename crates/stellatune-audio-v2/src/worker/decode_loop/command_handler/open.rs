use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_audio_core::pipeline::context::InputRef;

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::runtime::runner::RunnerState;
use crate::types::PlayerState;
use crate::worker::decode_loop::command_handler::control_apply;
use crate::worker::decode_loop::command_handler::gain_transition;
use crate::worker::decode_loop::loop_state::{DecodeLoopState, PrewarmedNext};
use crate::worker::decode_loop::pipeline_policies::apply_decode_policies;
use crate::worker::decode_loop::util::update_state;
use crate::worker::decode_loop::{DecodeLoopEvent, DecodeLoopEventCallback};

pub(crate) fn handle(
    input: InputRef,
    start_playing: bool,
    resp_tx: Sender<Result<(), String>>,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeLoopEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
) -> bool {
    let result = open_input(
        input,
        start_playing,
        assembler,
        callback,
        pipeline_runtime,
        state,
    );
    let _ = resp_tx.send(result);
    false
}

pub(crate) fn open_input(
    input: InputRef,
    start_playing: bool,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeLoopEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
) -> Result<(), String> {
    let transition = state.gain_transition;
    if let Some(active_runner) = state.runner.as_mut() {
        if state.state == PlayerState::Playing {
            let available_frames_hint = active_runner.playable_remaining_frames_hint();
            let _ = gain_transition::run_interrupt_fade_out(
                active_runner,
                &mut state.ctx,
                transition,
                transition.switch_fade_out_ms,
                available_frames_hint,
            );
        }
        active_runner.stop(&mut state.ctx);
    }
    state.runner = None;
    state.reset_context();
    state.active_input = None;
    state.queued_next_input = None;
    state.prewarmed_next = None;
    state.recovery_attempts = 0;
    state.recovery_retry_at = None;

    let plan = match state.pinned_plan.as_ref() {
        Some(plan) => Arc::clone(plan),
        None => assembler.plan(&input).map_err(|e| e.to_string())?,
    };
    let mut assembled = pipeline_runtime
        .ensure(plan.as_ref())
        .map_err(|e| e.to_string())?;
    apply_decode_policies(&mut assembled, state);
    let mut next_runner = assembled
        .into_runner(state.sink_latency, state.sink_control_timeout)
        .map_err(|e| e.to_string())?;
    next_runner
        .prepare(&input, &mut state.ctx)
        .map_err(|e| e.to_string())?;
    control_apply::apply_master_gain_level_to_runner(
        &mut next_runner,
        &mut state.ctx,
        state.master_gain_level,
    )?;
    control_apply::replay_persisted_stage_controls_to_runner(
        &state.persisted_stage_controls,
        &mut next_runner,
        &mut state.ctx,
    )?;
    if start_playing {
        gain_transition::request_fade_in_from_silence_with_runner(
            &mut next_runner,
            &mut state.ctx,
            transition,
            transition.open_fade_in_ms,
        )
        .map_err(|e| e.to_string())?;
    }
    next_runner.set_state(if start_playing {
        RunnerState::Playing
    } else {
        RunnerState::Paused
    });
    state.runner = Some(next_runner);
    state.active_input = Some(input.clone());
    state.last_position_emit_at = std::time::Instant::now();
    callback(DecodeLoopEvent::Position { position_ms: 0 });
    match input {
        InputRef::TrackToken(track_token) => {
            callback(DecodeLoopEvent::TrackChanged { track_token });
        },
    }
    update_state(
        callback,
        &mut state.state,
        if start_playing {
            PlayerState::Playing
        } else {
            PlayerState::Paused
        },
    );
    Ok(())
}

pub(crate) fn prewarm_input(
    input: InputRef,
    assembler: &Arc<dyn PipelineAssembler>,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &DecodeLoopState,
) -> Result<PrewarmedNext, String> {
    let plan = match state.pinned_plan.as_ref() {
        Some(plan) => Arc::clone(plan),
        None => assembler.plan(&input).map_err(|e| e.to_string())?,
    };
    let mut assembled = pipeline_runtime
        .ensure(plan.as_ref())
        .map_err(|e| e.to_string())?;
    apply_decode_policies(&mut assembled, state);
    let mut next_runner = assembled
        .into_runner(state.sink_latency, state.sink_control_timeout)
        .map_err(|e| e.to_string())?;
    let mut next_ctx = state.fresh_context();
    next_runner
        .prepare_decode(&input, &mut next_ctx)
        .map_err(|e| e.to_string())?;
    control_apply::replay_persisted_stage_controls_to_runner(
        &state.persisted_stage_controls,
        &mut next_runner,
        &mut next_ctx,
    )?;
    Ok(PrewarmedNext {
        input,
        runner: next_runner,
        ctx: next_ctx,
    })
}
