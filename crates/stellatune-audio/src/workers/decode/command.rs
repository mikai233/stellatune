use std::any::Any;
use std::sync::Arc;

use crossbeam_channel::Sender;
use stellatune_audio_core::pipeline::context::InputRef;

use crate::config::engine::{LfeMode, PauseBehavior, ResampleQuality, StopBehavior};
use crate::pipeline::assembly::{PipelineMutation, PipelinePlan};

pub(crate) enum DecodeWorkerCommand {
    Open {
        input: InputRef,
        start_playing: bool,
        resp_tx: Sender<Result<(), String>>,
    },
    QueueNext {
        input: InputRef,
        resp_tx: Sender<Result<(), String>>,
    },
    Play {
        resp_tx: Sender<Result<(), String>>,
    },
    Pause {
        behavior: PauseBehavior,
        resp_tx: Sender<Result<(), String>>,
    },
    Seek {
        position_ms: i64,
        resp_tx: Sender<Result<(), String>>,
    },
    Stop {
        behavior: StopBehavior,
        resp_tx: Sender<Result<(), String>>,
    },
    ApplyPipelinePlan {
        plan: Arc<dyn PipelinePlan>,
        resp_tx: Sender<Result<(), String>>,
    },
    ApplyPipelineMutation {
        mutation: PipelineMutation,
        resp_tx: Sender<Result<(), String>>,
    },
    SetLfeMode {
        mode: LfeMode,
        resp_tx: Sender<Result<(), String>>,
    },
    SetResampleQuality {
        quality: ResampleQuality,
        resp_tx: Sender<Result<(), String>>,
    },
    ApplyStageControl {
        stage_key: String,
        control: Box<dyn Any + Send>,
        resp_tx: Sender<Result<(), String>>,
    },
    Shutdown {
        ack_tx: Sender<()>,
    },
}
