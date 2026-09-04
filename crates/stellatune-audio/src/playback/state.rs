//! Actor-owned playback session and track pipeline state.
//!
//! `PlaybackSession` contains mutable domain state but deliberately excludes
//! [`PlaybackState`], which is owned by the Lattice behavior. `PreparedTrack`
//! has no output worker; activation converts it into `ActiveTrack` or, during
//! overlap, `SecondaryTrack`. Only `ActiveTrack` owns a `SinkWorker`.
//!
//! Frame counters have distinct coordinate systems: decoded counters include
//! encoder delay, audible counters exclude gapless trim, and sink counters are
//! relative to the current output worker. Explicit base fields translate these
//! values rather than treating them as interchangeable.

use std::sync::Arc;
use std::time::Instant;

use lattice_actor::reply::ReplyTo;

use crate::planner::{CrossfadeCurve, ExecutablePlaybackPlan, PlaybackPolicies, TransitionPolicy};
use stellatune_audio_core::{
    decoder::DecoderStage,
    error::PlaybackControlError,
    format::{AudioBlock, PcmFormat},
    playback::{MediaTime, PlaybackItemId},
    sink::SinkFactory,
    source::SourceCancellation,
    transform::TransformStage,
};

use super::event::PlaybackState;
use super::normalizer::PcmNormalizer;
use super::sink_worker::SinkWorker;
/// The domain role attached to one off-turn preparation task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreparationPurpose {
    Current {
        autoplay: bool,
    },
    Next,
    Recovery {
        item_id: PlaybackItemId,
        checkpoint: MediaTime,
        resume_state: PlaybackState,
        attempt: usize,
    },
}

/// The generation-tagged result returned from an off-turn preparation.
pub(super) struct PreparationResult {
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) purpose: PreparationPurpose,
    pub(super) result: Result<PreparedTrack, PlaybackControlError>,
}

/// The actor-side identity and deadline of current request-backed preparation.
pub(super) struct PendingPreparation {
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) purpose: PreparationPurpose,
    pub(super) deadline: Instant,
}

/// Recovery work retained so it can be scheduled through `pipe_to_self`.
pub(super) struct RecoveryPreparation {
    pub(super) plan: ExecutablePlaybackPlan,
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) purpose: PreparationPurpose,
    pub(super) cancellation: SourceCancellation,
}

/// A fully configured track pipeline that does not yet own an output worker.
pub(super) struct PreparedTrack {
    pub(super) plan: ExecutablePlaybackPlan,
    pub(super) decoder: Box<dyn DecoderStage>,
    pub(super) pre_mix_transforms: Vec<Box<dyn TransformStage>>,
    pub(super) pre_mix_formats: Vec<PcmFormat>,
    pub(super) post_mix_transforms: Vec<Box<dyn TransformStage>>,
    pub(super) post_mix_formats: Vec<PcmFormat>,
    pub(super) decoded_format: PcmFormat,
    pub(super) mix_format: PcmFormat,
    pub(super) output_format: PcmFormat,
    pub(super) normalizer: Option<PcmNormalizer>,
    pub(super) duration_frames: Option<u64>,
    pub(super) trim_head_frames: u64,
    pub(super) trim_tail_frames: u64,
    pub(super) raw_duration_frames: Option<u64>,
    pub(super) initial_decoded_frame: u64,
    pub(super) initial_audible_frame: u64,
}

/// The current track pipeline, including output, clocks, and transition state.
pub(super) struct ActiveTrack {
    pub(super) recovery_plan: ExecutablePlaybackPlan,
    pub(super) item_id: PlaybackItemId,
    pub(super) decoder: Box<dyn DecoderStage>,
    pub(super) pre_mix_transforms: Vec<Box<dyn TransformStage>>,
    pub(super) pre_mix_formats: Vec<PcmFormat>,
    pub(super) post_mix_transforms: Vec<Box<dyn TransformStage>>,
    pub(super) post_mix_formats: Vec<PcmFormat>,
    pub(super) decoded_format: PcmFormat,
    pub(super) mix_format: PcmFormat,
    pub(super) output_format: PcmFormat,
    pub(super) normalizer: Option<PcmNormalizer>,
    pub(super) duration_frames: Option<u64>,
    pub(super) trim_head_frames: u64,
    pub(super) trim_tail_frames: u64,
    pub(super) raw_duration_frames: Option<u64>,
    pub(super) tail_buffer: Vec<f32>,
    pub(super) decoded_frame: u64,
    pub(super) produced_audible_frame: u64,
    pub(super) position_base_frame: u64,
    pub(super) last_reported_position_frame: u64,
    pub(super) epoch: u64,
    pub(super) pending_block: Option<AudioBlock>,
    pub(super) sink_factory: Arc<dyn SinkFactory>,
    pub(super) output: SinkWorker,
    pub(super) sink_consumed_base_frame: u64,
    pub(super) boundary_announced: bool,
    pub(super) transition: TransitionPolicy,
    pub(super) fade_in_frames: u64,
    pub(super) fade_in_start_frame: u64,
    pub(super) recovery_fade: Option<TransitionRecoveryFade>,
    pub(super) seek_fade_frames: u64,
    pub(super) forced_end_frame: Option<u64>,
    pub(super) drain_phase: DrainPhase,
}

/// A gain ramp that restores the current track after a failed overlap.
#[derive(Debug, Clone, Copy)]
pub(super) struct TransitionRecoveryFade {
    pub(super) start_frame: u64,
    pub(super) duration_frames: u64,
    pub(super) start_gain: f32,
}

/// All mutable playback domain state exclusively owned by `PlaybackActor`.
pub(super) struct PlaybackSession {
    pub(super) generation: u64,
    pub(super) next_preparation_id: u64,
    pub(super) preparation_cancellation: SourceCancellation,
    pub(super) pending_preparation: Option<PendingPreparation>,
    pub(super) pending_recovery: Option<RecoveryPreparation>,
    pub(super) current: Option<ActiveTrack>,
    pub(super) next: Option<PreparedTrack>,
    pub(super) next_preparing: bool,
    pub(super) pending_seek: Option<PendingSeek>,
    pub(super) crossfade: Option<CrossfadeState>,
    pub(super) force_transition: bool,
    pub(super) policies: PlaybackPolicies,
    pub(super) output_gain: f32,
}

/// The next track while it is decoded concurrently for a crossfade.
pub(super) struct SecondaryTrack {
    pub(super) recovery_plan: ExecutablePlaybackPlan,
    pub(super) item_id: PlaybackItemId,
    pub(super) decoder: Box<dyn DecoderStage>,
    pub(super) pre_mix_transforms: Vec<Box<dyn TransformStage>>,
    pub(super) pre_mix_formats: Vec<PcmFormat>,
    pub(super) decoded_format: PcmFormat,
    pub(super) mix_format: PcmFormat,
    pub(super) normalizer: Option<PcmNormalizer>,
    pub(super) duration_frames: Option<u64>,
    pub(super) trim_head_frames: u64,
    pub(super) trim_tail_frames: u64,
    pub(super) raw_duration_frames: Option<u64>,
    pub(super) tail_buffer: Vec<f32>,
    pub(super) decoded_frame: u64,
    pub(super) produced_audible_frame: u64,
    pub(super) sink_factory: Arc<dyn SinkFactory>,
    pub(super) transition: TransitionPolicy,
    pub(super) seek_fade_frames: u64,
}

/// The stage currently being drained after decoder end-of-stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DrainPhase {
    Decoding,
    PreMix(usize),
    Normalizer,
    PostMix(usize),
    Complete,
}

/// Per-overlap cursors and pending blocks for two-track mixing.
pub(super) struct CrossfadeState {
    pub(super) next: SecondaryTrack,
    pub(super) duration_frames: u64,
    pub(super) curve: CrossfadeCurve,
    pub(super) progressed_frames: u64,
    pub(super) current_block: Option<AudioBlock>,
    pub(super) next_block: Option<AudioBlock>,
    pub(super) sink_consumed_base_frame: u64,
    pub(super) boundary_announced: bool,
}

/// A multi-turn decoder seek and the external reply waiting for completion.
pub(super) struct PendingSeek {
    pub(super) response: ReplyTo<Result<(), PlaybackControlError>>,
    pub(super) resume_state: PlaybackState,
    pub(super) item_id: PlaybackItemId,
}
