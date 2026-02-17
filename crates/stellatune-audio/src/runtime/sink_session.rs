use std::time::Duration;

use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;

use crate::assembly::SinkPlan;
use crate::types::SinkLatencyConfig;
use crate::workers::sink_worker::{SinkWorker, SinkWriteError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SinkActivationMode {
    // Switch away from current stream immediately. Old queued sink frames are dropped.
    ImmediateCutover,
    // Keep queued sink frames. Used for drained, seamless handover paths.
    PreserveQueued,
    // Force sink graph rebuild regardless of current stream spec.
    ForceRecreate,
}

pub(crate) struct SinkSession {
    sink_worker: Option<SinkWorker>,
    sink_spec: Option<StreamSpec>,
    sink_route_fingerprint: Option<u64>,
    sink_latency: SinkLatencyConfig,
    sink_control_timeout: Duration,
}

impl SinkSession {
    pub(crate) fn new(sink_latency: SinkLatencyConfig, sink_control_timeout: Duration) -> Self {
        Self {
            sink_worker: None,
            sink_spec: None,
            sink_route_fingerprint: None,
            sink_latency,
            sink_control_timeout,
        }
    }

    pub(crate) fn is_active_for(&self, spec: StreamSpec, route_fingerprint: u64) -> bool {
        self.sink_worker.is_some()
            && self.sink_spec == Some(spec)
            && self.sink_route_fingerprint == Some(route_fingerprint)
    }

    pub(crate) fn activate(
        &mut self,
        spec: StreamSpec,
        route_fingerprint: u64,
        sink_plan: &mut Option<Box<dyn SinkPlan>>,
        ctx: &PipelineContext,
        mode: SinkActivationMode,
    ) -> Result<bool, PipelineError> {
        if matches!(mode, SinkActivationMode::ForceRecreate) {
            self.shutdown(false);
        }

        if let Some(worker) = self.sink_worker.as_ref() {
            if matches!(mode, SinkActivationMode::ImmediateCutover) {
                match worker.drop_queued(self.sink_control_timeout) {
                    Ok(()) => {},
                    Err(PipelineError::SinkDisconnected) => {
                        self.sink_worker = None;
                        self.sink_spec = None;
                    },
                    Err(error) => return Err(error),
                }
            }
        }

        if self.sink_worker.is_some()
            && self.sink_spec == Some(spec)
            && self.sink_route_fingerprint == Some(route_fingerprint)
        {
            return Ok(true);
        }

        self.shutdown(false);
        let sink_plan = sink_plan
            .take()
            .ok_or_else(|| PipelineError::StageFailure("sink plan already consumed".to_string()))?;
        let sinks = sink_plan.into_sinks()?;
        let queue_capacity = self.sink_latency.queue_capacity(spec.sample_rate);
        let sink_worker = SinkWorker::start(sinks, spec, ctx.clone(), queue_capacity)?;
        self.sink_worker = Some(sink_worker);
        self.sink_spec = Some(spec);
        self.sink_route_fingerprint = Some(route_fingerprint);
        Ok(false)
    }

    pub(crate) fn try_send_block(&mut self, block: AudioBlock) -> Result<(), SinkWriteError> {
        let worker = self
            .sink_worker
            .as_mut()
            .ok_or(SinkWriteError::Disconnected)?;
        worker.try_send_block(block)
    }

    pub(crate) fn sync_runtime_control(&self, ctx: &PipelineContext) -> Result<(), PipelineError> {
        let worker = self
            .sink_worker
            .as_ref()
            .ok_or(PipelineError::NotPrepared)?;
        worker.sync_runtime_control(ctx, self.sink_control_timeout)
    }

    pub(crate) fn drop_queued(&self) -> Result<(), PipelineError> {
        let worker = self
            .sink_worker
            .as_ref()
            .ok_or(PipelineError::NotPrepared)?;
        worker.drop_queued(self.sink_control_timeout)
    }

    pub(crate) fn drain(&self) -> Result<(), PipelineError> {
        let worker = self
            .sink_worker
            .as_ref()
            .ok_or(PipelineError::NotPrepared)?;
        worker.drain(self.sink_control_timeout)
    }

    pub(crate) fn shutdown(&mut self, drain: bool) {
        if let Some(worker) = self.sink_worker.take() {
            let _ = worker.shutdown(drain, self.sink_control_timeout);
        }
        self.sink_spec = None;
        self.sink_route_fingerprint = None;
    }
}
