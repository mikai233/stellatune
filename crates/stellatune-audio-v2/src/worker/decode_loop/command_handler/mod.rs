mod apply_pipeline_plan;
mod apply_stage_control;
mod control_apply;
mod gain_transition;
#[cfg(test)]
mod integration_tests;
mod master_gain;
pub(crate) mod open;
mod pause;
mod play;
mod queue_next;
mod reconfigure_active;
mod seek;
mod set_dsp_chain;
mod set_lfe_mode;
mod set_resample_quality;
mod shutdown;
mod stop;

use std::sync::Arc;

use crate::assembly::{PipelineAssembler, PipelineRuntime};
use crate::worker::decode_loop::DecodeLoopEventCallback;
use crate::worker::decode_loop::command::DecodeLoopCommand;
use crate::worker::decode_loop::loop_state::DecodeLoopState;

pub(crate) use control_apply::apply_master_gain_level_to_runner;
pub(crate) use control_apply::replay_persisted_stage_controls_to_runner;
pub(crate) use gain_transition::request_fade_in_from_silence_with_runner;

pub(crate) fn handle_command(
    cmd: DecodeLoopCommand,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeLoopEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeLoopState,
) -> bool {
    match cmd {
        DecodeLoopCommand::Open {
            input,
            start_playing,
            resp_tx,
        } => open::handle(
            input,
            start_playing,
            resp_tx,
            assembler,
            callback,
            pipeline_runtime,
            state,
        ),
        DecodeLoopCommand::Play { resp_tx } => play::handle(resp_tx, callback, state),
        DecodeLoopCommand::QueueNext { input, resp_tx } => {
            queue_next::handle(input, resp_tx, assembler, pipeline_runtime, state)
        },
        DecodeLoopCommand::Pause { behavior, resp_tx } => {
            pause::handle(behavior, resp_tx, callback, state)
        },
        DecodeLoopCommand::Seek {
            position_ms,
            resp_tx,
        } => seek::handle(position_ms, resp_tx, callback, state),
        DecodeLoopCommand::Stop { behavior, resp_tx } => {
            stop::handle(behavior, resp_tx, callback, pipeline_runtime, state)
        },
        DecodeLoopCommand::ApplyPipelinePlan { plan, resp_tx } => {
            apply_pipeline_plan::handle(plan, resp_tx, callback, pipeline_runtime, state)
        },
        DecodeLoopCommand::SetMasterGainLevel { level, resp_tx } => {
            master_gain::handle(level, resp_tx, state)
        },
        DecodeLoopCommand::SetDspChain { spec, resp_tx } => {
            set_dsp_chain::handle(spec, resp_tx, pipeline_runtime)
        },
        DecodeLoopCommand::SetLfeMode { mode, resp_tx } => {
            set_lfe_mode::handle(mode, resp_tx, assembler, callback, pipeline_runtime, state)
        },
        DecodeLoopCommand::SetResampleQuality { quality, resp_tx } => set_resample_quality::handle(
            quality,
            resp_tx,
            assembler,
            callback,
            pipeline_runtime,
            state,
        ),
        DecodeLoopCommand::ApplyStageControl {
            stage_key,
            control,
            resp_tx,
        } => apply_stage_control::handle(stage_key, control, resp_tx, state),
        DecodeLoopCommand::Shutdown { ack_tx } => {
            shutdown::handle(ack_tx, callback, pipeline_runtime, state)
        },
    }
}
