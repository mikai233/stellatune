//! Pipeline runner state machine and step execution primitives.
//!
//! # Architecture
//!
//! [`PipelineRunner`] is the single owner of source/decoder/transform stages for
//! one active playback pipeline. It does not own sink devices directly; sink I/O
//! is coordinated through sink session handles.
//!
//! Runner lifecycle is split into three phases:
//! 1. `prepare_decode`: binds input and prepares decode/transform chain.
//! 2. `activate_sink`: attaches the prepared output spec to a sink session route.
//! 3. `step`/`drain`/`stop`: drives playback and teardown.
//!
//! # Invariants
//!
//! - `step` requires both decode preparation and a matching active sink route.
//! - At most one `pending_sink_block` exists; it is the backpressure bridge
//!   between decode pacing and sink queue capacity.
//! - Stage control dispatch is keyed by a validated stage-key map, so runtime
//!   controls never rely on transform index ordering from callers.

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
