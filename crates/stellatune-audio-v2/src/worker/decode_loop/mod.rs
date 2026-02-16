mod command;
mod command_handler;
mod loop_state;
mod main_loop;
mod pipeline_policies;
mod util;

use std::any::Any;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{SendTimeoutError, Sender};
use stellatune_audio_core::pipeline::context::InputRef;

use crate::assembly::{PipelineAssembler, PipelinePlan};
use crate::types::{
    DspChainSpec, EngineConfig, LfeMode, PauseBehavior, PlayerState, ResampleQuality, StopBehavior,
};
use crate::worker::decode_loop::command::DecodeLoopCommand;
use crate::worker::decode_loop::util::recv_result;

#[derive(Debug, Clone)]
pub(crate) enum DecodeLoopEvent {
    StateChanged(PlayerState),
    TrackChanged { track_token: String },
    Recovering { attempt: u32, backoff_ms: u64 },
    Position { position_ms: i64 },
    Eof,
    Error(String),
}

pub(crate) type DecodeLoopEventCallback = Arc<dyn Fn(DecodeLoopEvent) + Send + Sync>;

pub(crate) struct DecodeLoopWorker {
    tx: Sender<DecodeLoopCommand>,
    join: JoinHandle<()>,
}

impl DecodeLoopWorker {
    pub(crate) fn start(
        assembler: Arc<dyn PipelineAssembler>,
        config: EngineConfig,
        callback: DecodeLoopEventCallback,
    ) -> Self {
        let (tx, rx) =
            crossbeam_channel::bounded::<DecodeLoopCommand>(config.decode_command_capacity);
        let join = std::thread::Builder::new()
            .name("stellatune-audio-v2-decode-loop".to_string())
            .spawn(move || main_loop::decode_loop_main(assembler, config, callback, rx))
            .expect("failed to spawn decode loop");
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
            DecodeLoopCommand::Open {
                input: InputRef::TrackToken(track_token),
                start_playing,
                resp_tx,
            },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn play(&self, timeout: Duration) -> Result<(), String> {
        self.call_simple(|resp_tx| DecodeLoopCommand::Play { resp_tx }, timeout)
    }

    pub(crate) fn queue_next(&self, track_token: String, timeout: Duration) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeLoopCommand::QueueNext {
                input: InputRef::TrackToken(track_token),
                resp_tx,
            },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn pause(&self, behavior: PauseBehavior, timeout: Duration) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeLoopCommand::Pause { behavior, resp_tx },
            timeout,
        )
    }

    pub(crate) fn seek(&self, position_ms: i64, timeout: Duration) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(
            DecodeLoopCommand::Seek {
                position_ms,
                resp_tx,
            },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn stop(&self, behavior: StopBehavior, timeout: Duration) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeLoopCommand::Stop { behavior, resp_tx },
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
            DecodeLoopCommand::ApplyPipelinePlan { plan, resp_tx },
            timeout,
        )?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn set_master_gain_level(
        &self,
        level: f32,
        timeout: Duration,
    ) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeLoopCommand::SetMasterGainLevel { level, resp_tx },
            timeout,
        )
    }

    pub(crate) fn set_dsp_chain(
        &self,
        spec: DspChainSpec,
        timeout: Duration,
    ) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(DecodeLoopCommand::SetDspChain { spec, resp_tx }, timeout)?;
        recv_result(resp_rx, timeout)
    }

    pub(crate) fn set_lfe_mode(&self, mode: LfeMode, timeout: Duration) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeLoopCommand::SetLfeMode { mode, resp_tx },
            timeout,
        )
    }

    pub(crate) fn set_resample_quality(
        &self,
        quality: ResampleQuality,
        timeout: Duration,
    ) -> Result<(), String> {
        self.call_simple(
            |resp_tx| DecodeLoopCommand::SetResampleQuality { quality, resp_tx },
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
            DecodeLoopCommand::ApplyStageControl {
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
        self.send_command(DecodeLoopCommand::Shutdown { ack_tx }, timeout)?;
        ack_rx
            .recv_timeout(timeout)
            .map_err(|_| "decode loop shutdown timed out".to_string())?;
        self.join
            .join()
            .map_err(|_| "decode loop thread panicked".to_string())?;
        Ok(())
    }

    fn call_simple(
        &self,
        constructor: impl FnOnce(Sender<Result<(), String>>) -> DecodeLoopCommand,
        timeout: Duration,
    ) -> Result<(), String> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.send_command(constructor(resp_tx), timeout)?;
        recv_result(resp_rx, timeout)
    }

    fn send_command(&self, command: DecodeLoopCommand, timeout: Duration) -> Result<(), String> {
        self.tx
            .send_timeout(command, timeout)
            .map_err(|error| match error {
                SendTimeoutError::Timeout(_) => {
                    format!(
                        "decode loop command queue full after {}ms",
                        timeout.as_millis()
                    )
                },
                SendTimeoutError::Disconnected(_) => "decode loop exited".to_string(),
            })
    }
}
