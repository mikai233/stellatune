use std::sync::Arc;

use crate::config::engine::{LfeMode, PauseBehavior, ResampleQuality, StopBehavior};
use crate::error::EngineError;
use crate::pipeline::assembly::{PipelineBlueprint, PipelineMutation};
use crate::workers::decode::{DecodeWorker, DecodeWorkerEvent};
use stellatune_audio_core::pipeline::stages::{StageRuntimeUpdate, StageTarget};
use stellatune_runtime::thread_actor::Message;

pub(crate) struct InstallDecodeWorkerMessage {
    pub(crate) worker: DecodeWorker,
}

pub(crate) struct SwitchTrackMessage {
    pub(crate) track_token: String,
    pub(crate) autoplay: bool,
}
pub(crate) struct QueueNextTrackMessage {
    pub(crate) track_token: String,
}

pub(crate) struct PlayMessage;
pub(crate) struct PauseMessage {
    pub(crate) behavior: PauseBehavior,
}
pub(crate) struct StopMessage {
    pub(crate) behavior: StopBehavior,
}

pub(crate) struct SeekMessage {
    pub(crate) position_ms: i64,
}
pub(crate) struct SetLfeModeMessage {
    pub(crate) mode: LfeMode,
}
pub(crate) struct SetResampleQualityMessage {
    pub(crate) quality: ResampleQuality,
}
pub(crate) struct ApplyStageRuntimeUpdateMessage {
    pub(crate) target: StageTarget,
    pub(crate) update: Arc<dyn StageRuntimeUpdate>,
}

pub(crate) struct GetSnapshotMessage;
pub(crate) struct ShutdownMessage;
pub(crate) struct ApplyPipelineBlueprintMessage {
    pub(crate) blueprint: Arc<dyn PipelineBlueprint>,
}
pub(crate) struct ApplyPipelineMutationMessage {
    pub(crate) mutation: PipelineMutation,
}

pub(crate) struct OnDecodeWorkerEventMessage {
    pub(crate) event: DecodeWorkerEvent,
}

impl Message for InstallDecodeWorkerMessage {
    type Response = Result<(), EngineError>;
}

impl Message for SwitchTrackMessage {
    type Response = Result<(), EngineError>;
}

impl Message for QueueNextTrackMessage {
    type Response = Result<(), EngineError>;
}

impl Message for PlayMessage {
    type Response = Result<(), EngineError>;
}

impl Message for PauseMessage {
    type Response = Result<(), EngineError>;
}

impl Message for StopMessage {
    type Response = Result<(), EngineError>;
}

impl Message for SeekMessage {
    type Response = Result<(), EngineError>;
}

impl Message for SetLfeModeMessage {
    type Response = Result<(), EngineError>;
}

impl Message for SetResampleQualityMessage {
    type Response = Result<(), EngineError>;
}

impl Message for ApplyStageRuntimeUpdateMessage {
    type Response = Result<(), EngineError>;
}

impl Message for GetSnapshotMessage {
    type Response = crate::config::engine::EngineSnapshot;
}

impl Message for ShutdownMessage {
    type Response = Result<(), EngineError>;
}

impl Message for ApplyPipelineBlueprintMessage {
    type Response = Result<(), EngineError>;
}

impl Message for ApplyPipelineMutationMessage {
    type Response = Result<(), EngineError>;
}

impl Message for OnDecodeWorkerEventMessage {
    type Response = ();
}
