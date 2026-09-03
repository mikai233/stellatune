use std::sync::Arc;
use std::time::Instant;

use lattice_actor::reply::ReplyTo;

use crate::planner::{CrossfadeCurve, ExecutablePlaybackPlan, PlaybackPolicies, TransitionPolicy};
use stellatune_audio_core::{
    AudioBlock, DecoderStage, MediaTime, PcmFormat, PlaybackControlError, PlaybackItemId,
    SinkFactory, SourceCancellation, TransformStage,
};

use super::event::PlaybackState;
use super::normalizer::PcmNormalizer;
use super::sink_worker::SinkWorker;
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

pub(super) struct PreparationResult {
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) purpose: PreparationPurpose,
    pub(super) result: Result<PreparedTrack, PlaybackControlError>,
}

pub(super) struct PendingPreparation {
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) purpose: PreparationPurpose,
    pub(super) deadline: Instant,
}

pub(super) struct RecoveryPreparation {
    pub(super) plan: ExecutablePlaybackPlan,
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) purpose: PreparationPurpose,
    pub(super) cancellation: SourceCancellation,
}

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

#[derive(Debug, Clone, Copy)]
pub(super) struct TransitionRecoveryFade {
    pub(super) start_frame: u64,
    pub(super) duration_frames: u64,
    pub(super) start_gain: f32,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DrainPhase {
    Decoding,
    PreMix(usize),
    Normalizer,
    PostMix(usize),
    Complete,
}

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

pub(super) struct PendingSeek {
    pub(super) response: ReplyTo<Result<(), PlaybackControlError>>,
    pub(super) resume_state: PlaybackState,
    pub(super) item_id: PlaybackItemId,
}
