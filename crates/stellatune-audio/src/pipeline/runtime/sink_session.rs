use std::time::Duration;

use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;

use crate::config::sink::SinkLatencyConfig;
use crate::pipeline::assembly::SinkPlan;
use crate::workers::sink::worker::{SinkWorker, SinkWriteError};

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

        if let Some(worker) = self.sink_worker.as_ref()
            && matches!(mode, SinkActivationMode::ImmediateCutover)
        {
            match worker.drop_queued(self.sink_control_timeout) {
                Ok(()) => {},
                Err(PipelineError::SinkDisconnected) => {
                    self.sink_worker = None;
                    self.sink_spec = None;
                },
                Err(error) => return Err(error),
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
    use stellatune_audio_core::pipeline::error::PipelineError;
    use stellatune_audio_core::pipeline::stages::StageStatus;
    use stellatune_audio_core::pipeline::stages::sink::SinkStage;

    use crate::config::sink::SinkLatencyConfig;
    use crate::pipeline::assembly::{SinkPlan, StaticSinkPlan};
    use crate::pipeline::runtime::sink_session::{SinkActivationMode, SinkSession};
    use crate::workers::sink::worker::SinkWriteError;

    #[derive(Default)]
    struct TestSink;

    impl SinkStage for TestSink {
        fn prepare(
            &mut self,
            _spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn write(&mut self, _block: &AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            StageStatus::Ok
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    fn sink_plan() -> Option<Box<dyn SinkPlan>> {
        Some(Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])))
    }

    #[test]
    fn activate_reuses_active_sink_when_spec_and_route_match() {
        let mut session = SinkSession::new(SinkLatencyConfig::default(), Duration::from_millis(50));
        let mut plan = sink_plan();
        let spec = StreamSpec {
            sample_rate: 48_000,
            channels: 2,
        };
        let ctx = PipelineContext::default();

        let reused = session
            .activate(
                spec,
                7,
                &mut plan,
                &ctx,
                SinkActivationMode::ImmediateCutover,
            )
            .expect("initial activation should succeed");
        assert!(!reused);
        assert!(session.is_active_for(spec, 7));

        let mut no_plan: Option<Box<dyn SinkPlan>> = None;
        let reused = session
            .activate(
                spec,
                7,
                &mut no_plan,
                &ctx,
                SinkActivationMode::ImmediateCutover,
            )
            .expect("reuse activation should succeed");
        assert!(reused);
        session.shutdown(false);
    }

    #[test]
    fn force_recreate_requires_fresh_sink_plan() {
        let mut session = SinkSession::new(SinkLatencyConfig::default(), Duration::from_millis(50));
        let mut plan = sink_plan();
        let spec = StreamSpec {
            sample_rate: 48_000,
            channels: 2,
        };
        let ctx = PipelineContext::default();

        session
            .activate(
                spec,
                9,
                &mut plan,
                &ctx,
                SinkActivationMode::ImmediateCutover,
            )
            .expect("initial activation should succeed");

        let mut no_plan: Option<Box<dyn SinkPlan>> = None;
        let error = session
            .activate(
                spec,
                9,
                &mut no_plan,
                &ctx,
                SinkActivationMode::ForceRecreate,
            )
            .expect_err("force recreate without sink plan should fail");
        match error {
            PipelineError::StageFailure(message) => {
                assert!(message.contains("sink plan already consumed"));
            },
            other => panic!("unexpected error: {other:?}"),
        }

        let mut replacement_plan = sink_plan();
        let reused = session
            .activate(
                spec,
                9,
                &mut replacement_plan,
                &ctx,
                SinkActivationMode::ForceRecreate,
            )
            .expect("force recreate with replacement sink plan should succeed");
        assert!(!reused);
        session.shutdown(false);
    }

    #[test]
    fn operations_fail_when_sink_is_not_prepared() {
        let mut session = SinkSession::new(SinkLatencyConfig::default(), Duration::from_millis(50));
        let ctx = PipelineContext::default();
        let block = AudioBlock {
            channels: 2,
            samples: vec![0.0, 0.0],
        };

        assert!(matches!(
            session.sync_runtime_control(&ctx),
            Err(PipelineError::NotPrepared)
        ));
        assert!(matches!(
            session.drop_queued(),
            Err(PipelineError::NotPrepared)
        ));
        assert!(matches!(session.drain(), Err(PipelineError::NotPrepared)));
        assert!(matches!(
            session.try_send_block(block),
            Err(SinkWriteError::Disconnected)
        ));

        session.shutdown(false);
    }
}
