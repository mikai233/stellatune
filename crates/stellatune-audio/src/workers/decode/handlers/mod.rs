mod apply_pipeline_mutation;
mod apply_pipeline_plan;
mod apply_stage_control;
mod control_apply;
mod gain_transition;
#[cfg(test)]
mod integration_tests;
pub(crate) mod open;
mod pause;
mod play;
mod queue_next;
mod reconfigure_active;
mod seek;
mod set_lfe_mode;
mod set_resample_quality;
mod shutdown;
mod stop;

use std::sync::Arc;

use crate::pipeline::assembly::{PipelineAssembler, PipelineRuntime};
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::command::DecodeWorkerCommand;
use crate::workers::decode::state::DecodeWorkerState;

pub(crate) use control_apply::apply_master_gain_level_to_runner;
pub(crate) use control_apply::replay_persisted_stage_controls_to_runner;
pub(crate) use gain_transition::request_fade_in_from_silence_with_runner;

pub(crate) fn handle_command(
    cmd: DecodeWorkerCommand,
    assembler: &Arc<dyn PipelineAssembler>,
    callback: &DecodeWorkerEventCallback,
    pipeline_runtime: &mut dyn PipelineRuntime,
    state: &mut DecodeWorkerState,
) -> bool {
    match cmd {
        DecodeWorkerCommand::Open {
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
        DecodeWorkerCommand::Play { resp_tx } => play::handle(resp_tx, callback, state),
        DecodeWorkerCommand::QueueNext { input, resp_tx } => {
            queue_next::handle(input, resp_tx, assembler, pipeline_runtime, state)
        },
        DecodeWorkerCommand::Pause { behavior, resp_tx } => {
            pause::handle(behavior, resp_tx, callback, state)
        },
        DecodeWorkerCommand::Seek {
            position_ms,
            resp_tx,
        } => seek::handle(position_ms, resp_tx, callback, state),
        DecodeWorkerCommand::Stop { behavior, resp_tx } => {
            stop::handle(behavior, resp_tx, callback, pipeline_runtime, state)
        },
        DecodeWorkerCommand::ApplyPipelinePlan { plan, resp_tx } => {
            apply_pipeline_plan::handle(plan, resp_tx, callback, pipeline_runtime, state)
        },
        DecodeWorkerCommand::ApplyPipelineMutation { mutation, resp_tx } => {
            apply_pipeline_mutation::handle(
                mutation,
                resp_tx,
                assembler,
                callback,
                pipeline_runtime,
                state,
            )
        },
        DecodeWorkerCommand::SetLfeMode { mode, resp_tx } => {
            set_lfe_mode::handle(mode, resp_tx, assembler, callback, pipeline_runtime, state)
        },
        DecodeWorkerCommand::SetResampleQuality { quality, resp_tx } => {
            set_resample_quality::handle(
                quality,
                resp_tx,
                assembler,
                callback,
                pipeline_runtime,
                state,
            )
        },
        DecodeWorkerCommand::ApplyStageControl {
            stage_key,
            control,
            resp_tx,
        } => apply_stage_control::handle(stage_key, control, resp_tx, state),
        DecodeWorkerCommand::Shutdown { ack_tx } => {
            shutdown::handle(ack_tx, callback, pipeline_runtime, state)
        },
    }
}
