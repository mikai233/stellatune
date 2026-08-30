use crate::config::engine::{LfeMode, PauseBehavior, ResampleQuality, StopBehavior};
use crate::error::EngineError;
use crate::pipeline::plan::PlaybackCheckpoint;
use crate::workers::decode::DecodeWorkerEvent;

#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct SwitchTrackMessage {
    pub(crate) track_token: String,
    pub(crate) autoplay: bool,
}
#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct QueueNextTrackMessage {
    pub(crate) track_token: String,
}

#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct PlayMessage;
#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct PauseMessage {
    pub(crate) behavior: PauseBehavior,
}
#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct StopMessage {
    pub(crate) behavior: StopBehavior,
}

#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct SeekMessage {
    pub(crate) position_ms: i64,
}
#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct SetLfeModeMessage {
    pub(crate) mode: LfeMode,
}
#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct SetResampleQualityMessage {
    pub(crate) quality: ResampleQuality,
}
#[derive(lattice_actor::Request)]
#[request(response = crate::config::engine::EngineSnapshot)]
pub(crate) struct GetSnapshotMessage;
#[derive(lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct ShutdownMessage;
#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct RebuildPipelineMessage;

#[derive(lattice_actor::Message)]
pub(crate) struct OnDecodeWorkerEventMessage {
    pub(crate) event: DecodeWorkerEvent,
}

#[derive(Debug, lattice_actor::Message)]
pub(crate) struct PumpAudioMessage;

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<Option<PlaybackCheckpoint>, EngineError>)]
pub(crate) struct SuspendForPluginChangeMessage;

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct CompletePluginChangeMessage;

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<(), EngineError>)]
pub(crate) struct AbortPluginChangeMessage;
