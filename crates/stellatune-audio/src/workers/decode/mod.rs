mod command;
mod handlers;
mod pipeline_policies;
mod recovery;
mod state;
mod util;
#[path = "loop.rs"]
mod worker_loop;

use std::any::Any;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{SendTimeoutError, Sender};
use stellatune_audio_core::pipeline::context::InputRef;

use crate::config::engine::{
    EngineConfig, LfeMode, PauseBehavior, PlayerState, ResampleQuality, StopBehavior,
};
use crate::pipeline::assembly::{PipelineAssembler, PipelineMutation, PipelinePlan};
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;
use crate::workers::decode::command::DecodeWorkerCommand;
use crate::workers::decode::util::recv_result;

#[derive(Debug, Clone)]
pub(crate) enum DecodeWorkerEvent {
    StateChanged(PlayerState),
    TrackChanged { track_token: String },
    Recovering { attempt: u32, backoff_ms: u64 },
    Position { position_ms: i64 },
    Eof,
    Error(String),
}

pub(crate) type DecodeWorkerEventCallback = Arc<dyn Fn(DecodeWorkerEvent) + Send + Sync>;

pub(crate) struct DecodeWorker {
    tx: Sender<DecodeWorkerCommand>,
    join: JoinHandle<()>,
}

impl DecodeWorker {
    pub(crate) fn start(
        assembler: Arc<dyn PipelineAssembler>,
        config: EngineConfig,
        callback: DecodeWorkerEventCallback,
        master_gain_hot_control: SharedMasterGainHotControl,
    ) -> Self {
        let (tx, rx) =
            crossbeam_channel::bounded::<DecodeWorkerCommand>(config.decode_command_capacity);
        let join = std::thread::Builder::new()
            .name("stellatune-audio-decode-worker".to_string())
            .spawn(move || {
                let _rt_guard = crate::infra::realtime::enable_realtime_audio_thread();
                worker_loop::decode_worker_main(
                    assembler,
                    config,
                    callback,
                    rx,
                    master_gain_hot_control,
                )
            })
            .expect("failed to spawn decode worker");
        Self { tx, join }
    }

    pub(crate) fn open(
        &self,
        track_token: String,
        start_playing: bool,
        timeout: Duration,
    ) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeWorkerCommand::Open {
                input: InputRef::TrackToken(track_token),
                start_playing,
                resp_tx,
            },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn play(&self, timeout: Duration) -> Result<(), String> {
        self.call_simple(|resp_tx| DecodeWorkerCommand::Play { resp_tx }, timeout)
    }

    pub(crate) fn queue_next(&self, track_token: String, timeout: Duration) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeWorkerCommand::QueueNext {
                input: InputRef::TrackToken(track_token),
                resp_tx,
            },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn pause(&self, behavior: PauseBehavior, timeout: Duration) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeWorkerCommand::Pause { behavior, resp_tx },
            timeout,
        )
    }

    pub(crate) fn seek(&self, position_ms: i64, timeout: Duration) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeWorkerCommand::Seek {
                position_ms,
                resp_tx,
            },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn stop(&self, behavior: StopBehavior, timeout: Duration) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeWorkerCommand::Stop { behavior, resp_tx },
            timeout,
        )
    }

    pub(crate) fn apply_pipeline_plan(
        &self,
        plan: Arc<dyn PipelinePlan>,
        timeout: Duration,
    ) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeWorkerCommand::ApplyPipelinePlan { plan, resp_tx },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn apply_pipeline_mutation(
        &self,
        mutation: PipelineMutation,
        timeout: Duration,
    ) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeWorkerCommand::ApplyPipelineMutation { mutation, resp_tx },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn set_lfe_mode(&self, mode: LfeMode, timeout: Duration) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeWorkerCommand::SetLfeMode { mode, resp_tx },
            timeout,
        )
    }

    pub(crate) fn set_resample_quality(
        &self,
        quality: ResampleQuality,
        timeout: Duration,
    ) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeWorkerCommand::SetResampleQuality { quality, resp_tx },
            timeout,
        )
    }

    pub(crate) fn apply_stage_control(
        &self,
        stage_key: impl Into<String>,
        control: Box<dyn Any + Send>,
        timeout: Duration,
    ) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeWorkerCommand::ApplyStageControl {
                stage_key: stage_key.into(),
                control,
                resp_tx,
            },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn shutdown(self, timeout: Duration) -> Result<(), String> {
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        self.send_command(DecodeWorkerCommand::Shutdown { ack_tx }, timeout)?;
        ack_rx
            .recv_timeout(timeout)
            .map_err(|_| "decode worker shutdown timed out".to_string())?;
        self.join
            .join()
            .map_err(|_| "decode worker thread panicked".to_string())?;
        Ok(())
    }

    fn call_simple(
        &self,
        constructor: impl FnOnce(Sender<Result<(), String>>) -> DecodeWorkerCommand,
        timeout: Duration,
    ) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(constructor(resp_tx), timeout)?;
        recv_result(resp_rx, timeout)
    }

    fn send_command(&self, command: DecodeWorkerCommand, timeout: Duration) -> Result<(), String> {
        self.tx
            .send_timeout(command, timeout)
            .map_err(|error| match error {
                SendTimeoutError::Timeout(_) => {
                    format!(
                        "decode worker command queue full after {}ms",
                        timeout.as_millis()
                    )
                },
                SendTimeoutError::Disconnected(_) => "decode worker exited".to_string(),
            })
    }
}
