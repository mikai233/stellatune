use std::collections::HashMap;

#[cfg(test)]
use std::sync::{Arc, Mutex};

#[cfg(test)]
use stellatune_audio_core::pipeline::context::GainTransitionRequest;
use stellatune_audio_core::pipeline::context::{
    AudioBlock, GaplessTrimSpec, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::pipeline::assembly::SinkPlan;

mod control;
mod lifecycle;
mod step;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunnerState {
    Stopped,
    Paused,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepResult {
    Idle,
    Produced { frames: usize },
    Eof,
}

const MAX_DRAIN_TAIL_ITERATIONS: usize = 32;
const MAX_PENDING_SINK_FLUSH_ATTEMPTS: usize = 32;

pub(crate) struct PipelineRunner {
    source: Box<dyn SourceStage>,
    decoder: Box<dyn DecoderStage>,
    transforms: Vec<Box<dyn TransformStage>>,
    supports_transition_gain: bool,
    supports_gapless_trim: bool,
    sink_plan: Option<Box<dyn SinkPlan>>,
    sink_route_fingerprint: u64,
    pending_sink_block: Option<AudioBlock>,
    source_handle: Option<SourceHandle>,
    decoder_spec: Option<StreamSpec>,
    output_spec: Option<StreamSpec>,
    decoder_gapless_trim_spec: Option<GaplessTrimSpec>,
    playable_remaining_frames_hint: Option<u64>,
    transform_control_routes: HashMap<String, usize>,
    #[cfg(test)]
    transition_request_log_sink: Option<Arc<Mutex<Vec<GainTransitionRequest>>>>,
    state: RunnerState,
}

#[path = "../../../tests/pipeline/runtime_runner.rs"]
mod tests;
