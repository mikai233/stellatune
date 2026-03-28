use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_audio_core::pipeline::context::InputRef;

use crate::config::engine::{LfeMode, PauseBehavior, ResampleQuality, StopBehavior};
use crate::error::DecodeError;
use crate::pipeline::assembly::{PipelineBlueprint, PipelineMutation};
use stellatune_audio_core::pipeline::stages::{StageRuntimeUpdate, StageTarget};

pub(crate) enum DecodeWorkerCommand {
    Open {
        input: InputRef,
        start_playing: bool,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    QueueNext {
        input: InputRef,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    Play {
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    Pause {
        behavior: PauseBehavior,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    Seek {
        position_ms: i64,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    Stop {
        behavior: StopBehavior,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    ApplyPipelineBlueprint {
        blueprint: Arc<dyn PipelineBlueprint>,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    ApplyPipelineMutation {
        mutation: PipelineMutation,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    ApplyPipelineMutations {
        mutations: Vec<PipelineMutation>,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    SetLfeMode {
        mode: LfeMode,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    SetResampleQuality {
        quality: ResampleQuality,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    ApplyStageRuntimeUpdate {
        target: StageTarget,
        update: Arc<dyn StageRuntimeUpdate>,
        resp_tx: Sender<Result<(), DecodeError>>,
    },
    Shutdown {
        ack_tx: Sender<()>,
    },
}
