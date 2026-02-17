use std::any::Any;
use std::sync::Arc;

use crate::config::engine::{LfeMode, PauseBehavior, ResampleQuality, StopBehavior};
use crate::pipeline::assembly::{PipelineMutation, PipelinePlan};
use crate::workers::decode::{DecodeWorker, DecodeWorkerEvent};
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
pub(crate) struct ApplyStageControlMessage {
    pub(crate) stage_key: String,
    pub(crate) control: Box<dyn Any + Send>,
}

pub(crate) struct GetSnapshotMessage;
pub(crate) struct ShutdownMessage;
pub(crate) struct ApplyPipelinePlanMessage {
    pub(crate) plan: Arc<dyn PipelinePlan>,
}
pub(crate) struct ApplyPipelineMutationMessage {
    pub(crate) mutation: PipelineMutation,
}

pub(crate) struct OnDecodeWorkerEventMessage {
    pub(crate) event: DecodeWorkerEvent,
}

impl Message for InstallDecodeWorkerMessage {
    type Response = Result<(), String>;
}

impl Message for SwitchTrackMessage {
    type Response = Result<(), String>;
}

impl Message for QueueNextTrackMessage {
    type Response = Result<(), String>;
}

impl Message for PlayMessage {
    type Response = Result<(), String>;
}

impl Message for PauseMessage {
    type Response = Result<(), String>;
}

impl Message for StopMessage {
    type Response = Result<(), String>;
}

impl Message for SeekMessage {
    type Response = Result<(), String>;
}

impl Message for SetLfeModeMessage {
    type Response = Result<(), String>;
}

impl Message for SetResampleQualityMessage {
    type Response = Result<(), String>;
}

impl Message for ApplyStageControlMessage {
    type Response = Result<(), String>;
}

impl Message for GetSnapshotMessage {
    type Response = crate::config::engine::EngineSnapshot;
}

impl Message for ShutdownMessage {
    type Response = Result<(), String>;
}

impl Message for ApplyPipelinePlanMessage {
    type Response = Result<(), String>;
}

impl Message for ApplyPipelineMutationMessage {
    type Response = Result<(), String>;
}

impl Message for OnDecodeWorkerEventMessage {
    type Response = ();
}
