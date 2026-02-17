use std::any::Any;
use std::collections::HashMap;
use std::time::Duration;

#[cfg(test)]
use stellatune_audio_core::pipeline::context::GainTransitionRequest;
use stellatune_audio_core::pipeline::context::{
    AudioBlock, GaplessTrimSpec, InputRef, PipelineContext, SourceHandle, StreamSpec,
};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;

use crate::assembly::SinkPlan;
use crate::runtime::sink_session::{SinkActivationMode, SinkSession};
#[cfg(test)]
use crate::runtime::transform::control::TransitionGainControl;
use crate::runtime::transform::control::{GAPLESS_TRIM_STAGE_KEY, GaplessTrimControl};
use crate::types::{PauseBehavior, SinkLatencyConfig, StopBehavior};
use crate::workers::sink_worker::SinkWriteError;
#[cfg(test)]
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunnerState {
    Stopped,
    Paused,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepResult {
    Idle,
    Produced { frames: usize },
    Eof,
}

const MAX_DRAIN_TAIL_ITERATIONS: usize = 32;
const MAX_PENDING_SINK_FLUSH_ATTEMPTS: usize = 32;

pub(crate) struct PipelineRunner {
    source: Box<dyn SourceStage>,
    decoder: Box<dyn DecoderStage>,
    transforms: Vec<Box<dyn TransformStage>>,
    supports_transition_gain: bool,
    supports_gapless_trim: bool,
    sink_plan: Option<Box<dyn SinkPlan>>,
    sink_route_fingerprint: u64,
    pending_sink_block: Option<AudioBlock>,
    source_handle: Option<SourceHandle>,
    decoder_spec: Option<StreamSpec>,
    output_spec: Option<StreamSpec>,
    decoder_gapless_trim_spec: Option<GaplessTrimSpec>,
    playable_remaining_frames_hint: Option<u64>,
    transform_control_routes: HashMap<String, usize>,
    #[cfg(test)]
    transition_request_log_sink: Option<Arc<Mutex<Vec<GainTransitionRequest>>>>,
    state: RunnerState,
}

impl PipelineRunner {
    #[allow(dead_code)]
    pub(crate) fn new(
        source: Box<dyn SourceStage>,
        decoder: Box<dyn DecoderStage>,
        transforms: Vec<Box<dyn TransformStage>>,
        sink_plan: Box<dyn SinkPlan>,
        _sink_latency: SinkLatencyConfig,
        _sink_control_timeout: Duration,
        supports_transition_gain: bool,
        supports_gapless_trim: bool,
    ) -> Result<Self, PipelineError> {
        let sink_route_fingerprint = sink_plan.route_fingerprint();
        let transform_control_routes = Self::build_transform_control_routes(&transforms)?;
        Ok(Self {
            source,
            decoder,
            transforms,
            supports_transition_gain,
            supports_gapless_trim,
            sink_plan: Some(sink_plan),
            sink_route_fingerprint,
            pending_sink_block: None,
            source_handle: None,
            decoder_spec: None,
            output_spec: None,
            decoder_gapless_trim_spec: None,
            playable_remaining_frames_hint: None,
            transform_control_routes,
            #[cfg(test)]
            transition_request_log_sink: None,
            state: RunnerState::Stopped,
        })
    }

    pub(crate) fn prepare_decode(
        &mut self,
        input: &InputRef,
        ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        if self.source_handle.is_some() || self.output_spec.is_some() {
            return Err(PipelineError::StageFailure(
                "decode already prepared".to_string(),
            ));
        }
        let source_handle = self.source.prepare(input, ctx)?;
        let decoder_spec = self.decoder.prepare(&source_handle, ctx)?.validate()?;
        let decoder_gapless_trim_spec =
            Self::normalize_gapless_trim_spec(self.decoder.current_gapless_trim_spec());
        let mut spec = decoder_spec;
        for transform in &mut self.transforms {
            spec = transform.prepare(spec, ctx)?.validate()?;
        }

        self.source_handle = Some(source_handle);
        self.decoder_spec = Some(decoder_spec);
        self.output_spec = Some(spec);
        self.decoder_gapless_trim_spec = decoder_gapless_trim_spec;
        self.pending_sink_block = None;
        self.apply_gapless_trim_control(ctx)?;
        self.refresh_playable_remaining_frames_hint();
        Ok(spec)
    }

    pub(crate) fn activate_sink(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &PipelineContext,
        mode: SinkActivationMode,
    ) -> Result<bool, PipelineError> {
        self.ensure_decode_prepared()?;
        let spec = self.output_spec.ok_or(PipelineError::NotPrepared)?;
        let reused = sink_session.activate(
            spec,
            self.sink_route_fingerprint,
            &mut self.sink_plan,
            ctx,
            mode,
        )?;
        self.pending_sink_block = None;
        Ok(reused)
    }

    pub(crate) fn drain_sink_for_reuse(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        self.drain(sink_session, ctx)
    }

    pub(crate) fn set_state(&mut self, state: RunnerState) {
        self.state = state;
    }

    pub(crate) fn state(&self) -> RunnerState {
        self.state
    }

    pub(crate) fn has_pending_sink_block(&self) -> bool {
        self.pending_sink_block.is_some()
    }

    pub(crate) fn supports_transition_gain(&self) -> bool {
        self.supports_transition_gain
    }

    pub(crate) fn supports_gapless_trim(&self) -> bool {
        self.supports_gapless_trim
    }

    pub(crate) fn playable_remaining_frames_hint(&self) -> Option<u64> {
        self.playable_remaining_frames_hint
    }

    pub(crate) fn output_sample_rate(&self) -> Option<u32> {
        self.output_spec.map(|spec| spec.sample_rate)
    }

    pub(crate) fn pause(
        &mut self,
        behavior: PauseBehavior,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        if matches!(behavior, PauseBehavior::DrainSink) {
            self.drain(sink_session, ctx)?;
        }
        self.state = RunnerState::Paused;
        Ok(())
    }

    pub(crate) fn seek(
        &mut self,
        position_ms: i64,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        self.pending_sink_block = None;
        sink_session.drop_queued()?;
        ctx.request_seek(position_ms);
        self.refresh_playable_remaining_frames_hint();
        Ok(())
    }

    pub(crate) fn sync_runtime_control(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        self.source.sync_runtime_control(ctx)?;
        self.decoder.sync_runtime_control(ctx)?;
        let next_gapless_trim_spec =
            Self::normalize_gapless_trim_spec(self.decoder.current_gapless_trim_spec());
        if next_gapless_trim_spec != self.decoder_gapless_trim_spec {
            self.decoder_gapless_trim_spec = next_gapless_trim_spec;
            self.apply_gapless_trim_control(ctx)?;
        }
        for transform in &mut self.transforms {
            transform.sync_runtime_control(ctx)?;
        }
        sink_session.sync_runtime_control(ctx)?;
        Ok(())
    }

    pub(crate) fn apply_transform_control_to(
        &mut self,
        stage_key: &str,
        control: &dyn Any,
        ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        self.ensure_decode_prepared()?;
        self.apply_transform_control_internal(stage_key, control, ctx)
    }

    fn apply_transform_control_internal(
        &mut self,
        stage_key: &str,
        control: &dyn Any,
        ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        let Some(target_index) = self.transform_control_routes.get(stage_key).copied() else {
            return Ok(false);
        };
        let transforms_len = self.transforms.len();
        let transform = self.transforms.get_mut(target_index).ok_or_else(|| {
            PipelineError::StageFailure(format!(
                "transform control target out of bounds: key={stage_key}, index={target_index}, len={transforms_len}"
            ))
        })?;
        let handled = transform.apply_control(control, ctx)?;
        if !handled {
            return Err(PipelineError::StageFailure(format!(
                "transform control target rejected control: key={stage_key}, index={target_index}"
            )));
        }
        #[cfg(test)]
        if let Some(control) = control.downcast_ref::<TransitionGainControl>() {
            if let Some(sink) = self.transition_request_log_sink.as_ref() {
                sink.lock()
                    .expect("transition request log sink mutex poisoned")
                    .push(control.request);
            }
        }
        Ok(true)
    }

    pub(crate) fn step(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<StepResult, PipelineError> {
        self.ensure_sink_prepared(sink_session)?;
        if self.state != RunnerState::Playing {
            return Ok(StepResult::Idle);
        }

        self.sync_runtime_control(sink_session, ctx)?;
        if let Some(seek_ms) = ctx.clear_pending_seek() {
            ctx.position_ms = seek_ms;
        }
        self.refresh_playable_remaining_frames_hint();

        let out_spec = self.output_spec.ok_or(PipelineError::NotPrepared)?;
        if let Some(block) = self.pending_sink_block.take() {
            return self.try_push_sink_block(sink_session, block, out_spec, ctx);
        }
        let mut block = AudioBlock::new(out_spec.channels);

        match self.decoder.next_block(&mut block, ctx) {
            StageStatus::Ok => {},
            StageStatus::Eof => {
                self.playable_remaining_frames_hint = Some(0);
                return Ok(StepResult::Eof);
            },
            StageStatus::Fatal => {
                return Err(PipelineError::StageFailure("decoder fatal".to_string()));
            },
        }
        self.refresh_playable_remaining_frames_hint();
        if block.is_empty() {
            return Ok(StepResult::Idle);
        }

        for transform in &mut self.transforms {
            match transform.process(&mut block, ctx) {
                StageStatus::Ok => {},
                StageStatus::Eof => {
                    self.playable_remaining_frames_hint = Some(0);
                    return Ok(StepResult::Eof);
                },
                StageStatus::Fatal => {
                    return Err(PipelineError::StageFailure("transform fatal".to_string()));
                },
            }
        }
        if block.is_empty() {
            return Ok(StepResult::Idle);
        }

        self.try_push_sink_block(sink_session, block, out_spec, ctx)
    }

    pub(crate) fn stop(&mut self, sink_session: &mut SinkSession, ctx: &mut PipelineContext) {
        sink_session.shutdown(false);
        self.stop_decode_only(ctx);
    }

    pub(crate) fn stop_decode_only(&mut self, ctx: &mut PipelineContext) {
        self.pending_sink_block = None;
        self.playable_remaining_frames_hint = None;
        self.decoder_gapless_trim_spec = None;
        for transform in &mut self.transforms {
            transform.stop(ctx);
        }
        self.decoder.stop(ctx);
        self.source.stop(ctx);

        self.source_handle = None;
        self.decoder_spec = None;
        self.output_spec = None;
        self.state = RunnerState::Stopped;
    }

    pub(crate) fn stop_with_behavior(
        &mut self,
        behavior: StopBehavior,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        if matches!(behavior, StopBehavior::DrainSink) && self.is_decode_prepared() {
            self.drain(sink_session, ctx)?;
        }
        self.stop(sink_session, ctx);
        Ok(())
    }

    fn drain(
        &mut self,
        sink_session: &mut SinkSession,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        self.decoder.flush(ctx)?;
        for transform in &mut self.transforms {
            transform.flush(ctx)?;
        }
        let out_spec = self.output_spec.ok_or(PipelineError::NotPrepared)?;
        self.flush_pending_sink_blocks(sink_session, out_spec, ctx)?;
        self.drain_transform_tail(sink_session, out_spec, ctx)?;
        self.flush_pending_sink_blocks(sink_session, out_spec, ctx)?;
        sink_session.drain()?;
        Ok(())
    }

    fn try_push_sink_block(
        &mut self,
        sink_session: &mut SinkSession,
        block: AudioBlock,
        out_spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<StepResult, PipelineError> {
        let produced_frames = block.frames();
        match sink_session.try_send_block(block) {
            Ok(()) => {
                ctx.advance_frames(produced_frames as u64, out_spec.sample_rate);
                Ok(StepResult::Produced {
                    frames: produced_frames,
                })
            },
            Err(SinkWriteError::Full(block)) => {
                self.pending_sink_block = Some(block);
                Ok(StepResult::Idle)
            },
            Err(SinkWriteError::Disconnected) => Err(PipelineError::SinkDisconnected),
        }
    }

    fn ensure_sink_prepared(&self, sink_session: &SinkSession) -> Result<(), PipelineError> {
        let output_spec = self.output_spec.ok_or(PipelineError::NotPrepared)?;
        if !self.is_decode_prepared()
            || !sink_session.is_active_for(output_spec, self.sink_route_fingerprint)
        {
            return Err(PipelineError::NotPrepared);
        }
        Ok(())
    }

    fn ensure_decode_prepared(&self) -> Result<(), PipelineError> {
        if !self.is_decode_prepared() {
            return Err(PipelineError::NotPrepared);
        }
        Ok(())
    }

    fn flush_pending_sink_blocks(
        &mut self,
        sink_session: &mut SinkSession,
        out_spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        let mut attempts = 0usize;
        while let Some(block) = self.pending_sink_block.take() {
            match self.try_push_sink_block(sink_session, block, out_spec, ctx)? {
                StepResult::Produced { .. } => {},
                StepResult::Idle => {
                    attempts = attempts.saturating_add(1);
                    if attempts >= MAX_PENDING_SINK_FLUSH_ATTEMPTS {
                        return Err(PipelineError::StageFailure(
                            "pending sink block could not be drained".to_string(),
                        ));
                    }
                    sink_session.drain()?;
                },
                StepResult::Eof => unreachable!("try_push_sink_block never returns eof"),
            }
        }
        Ok(())
    }

    fn drain_transform_tail(
        &mut self,
        sink_session: &mut SinkSession,
        out_spec: StreamSpec,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        for _ in 0..MAX_DRAIN_TAIL_ITERATIONS {
            let mut block = AudioBlock::new(out_spec.channels);
            for transform in &mut self.transforms {
                match transform.process(&mut block, ctx) {
                    StageStatus::Ok => {},
                    StageStatus::Eof => return Ok(()),
                    StageStatus::Fatal => {
                        return Err(PipelineError::StageFailure("transform fatal".to_string()));
                    },
                }
            }

            if block.is_empty() {
                break;
            }

            match self.try_push_sink_block(sink_session, block, out_spec, ctx)? {
                StepResult::Produced { .. } => {},
                StepResult::Idle => {
                    sink_session.drain()?;
                    self.flush_pending_sink_blocks(sink_session, out_spec, ctx)?;
                },
                StepResult::Eof => unreachable!("try_push_sink_block never returns eof"),
            }
        }
        Ok(())
    }

    fn scale_decoder_frames_to_output_domain(&self, frames: u64) -> u64 {
        let Some(decoder_spec) = self.decoder_spec else {
            return frames;
        };
        let Some(output_spec) = self.output_spec else {
            return frames;
        };
        let decoder_rate = decoder_spec.sample_rate.max(1) as u128;
        let output_rate = output_spec.sample_rate.max(1) as u128;
        if decoder_rate == output_rate {
            return frames;
        }
        let scaled = (frames as u128).saturating_mul(output_rate) / decoder_rate;
        scaled.min(u64::MAX as u128) as u64
    }

    fn refresh_playable_remaining_frames_hint(&mut self) {
        let tail_frames = if self.supports_gapless_trim() {
            self.decoder_gapless_trim_spec
                .map(|v| v.tail_frames as u64)
                .unwrap_or(0)
        } else {
            0
        };
        let hint = self.decoder.estimated_remaining_frames().map(|frames| {
            let playable_decoder_frames = frames.saturating_sub(tail_frames);
            self.scale_decoder_frames_to_output_domain(playable_decoder_frames)
        });
        self.playable_remaining_frames_hint = hint;
    }

    fn apply_gapless_trim_control(
        &mut self,
        ctx: &mut PipelineContext,
    ) -> Result<(), PipelineError> {
        if !self.supports_gapless_trim {
            return Ok(());
        }
        let control = GaplessTrimControl::new(self.decoder_gapless_trim_spec, ctx.position_ms);
        let _ = self.apply_transform_control_internal(GAPLESS_TRIM_STAGE_KEY, &control, ctx)?;
        Ok(())
    }

    fn normalize_gapless_trim_spec(spec: Option<GaplessTrimSpec>) -> Option<GaplessTrimSpec> {
        spec.filter(|v| !v.is_disabled())
    }

    fn build_transform_control_routes(
        transforms: &[Box<dyn TransformStage>],
    ) -> Result<HashMap<String, usize>, PipelineError> {
        let mut routes = HashMap::new();
        for (index, transform) in transforms.iter().enumerate() {
            if let Some(stage_key) = transform.stage_key() {
                let key = stage_key.trim();
                if key.is_empty() {
                    return Err(PipelineError::StageFailure(
                        "transform stage key must not be empty".to_string(),
                    ));
                }
                if routes.insert(key.to_string(), index).is_some() {
                    return Err(PipelineError::StageFailure(format!(
                        "duplicate transform stage key: {key}"
                    )));
                }
            }
        }
        Ok(routes)
    }

    #[cfg(test)]
    pub(crate) fn set_transition_request_log_sink(
        &mut self,
        sink: Arc<Mutex<Vec<GainTransitionRequest>>>,
    ) {
        self.transition_request_log_sink = Some(sink);
    }

    fn is_decode_prepared(&self) -> bool {
        self.source_handle.is_some() && self.output_spec.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::PipelineRunner;
    use crate::assembly::StaticSinkPlan;
    use crate::types::SinkLatencyConfig;
    use stellatune_audio_core::pipeline::context::{
        AudioBlock, GaplessTrimSpec, InputRef, PipelineContext, SourceHandle, StreamSpec,
    };
    use stellatune_audio_core::pipeline::error::PipelineError;
    use stellatune_audio_core::pipeline::stages::StageStatus;
    use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
    use stellatune_audio_core::pipeline::stages::sink::SinkStage;
    use stellatune_audio_core::pipeline::stages::source::SourceStage;
    use stellatune_audio_core::pipeline::stages::transform::TransformStage;

    #[derive(Default)]
    struct TestSource;

    impl SourceStage for TestSource {
        fn prepare(
            &mut self,
            _input: &InputRef,
            _ctx: &mut PipelineContext,
        ) -> Result<SourceHandle, PipelineError> {
            Ok(SourceHandle::new(()))
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    struct TestDecoder {
        remaining_frames: u64,
        gapless_trim_spec: Option<GaplessTrimSpec>,
    }

    impl TestDecoder {
        fn new(remaining_frames: u64, gapless_trim_spec: Option<GaplessTrimSpec>) -> Self {
            Self {
                remaining_frames,
                gapless_trim_spec,
            }
        }
    }

    impl DecoderStage for TestDecoder {
        fn prepare(
            &mut self,
            _source: &SourceHandle,
            _ctx: &mut PipelineContext,
        ) -> Result<StreamSpec, PipelineError> {
            Ok(StreamSpec {
                sample_rate: 1_000,
                channels: 1,
            })
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn current_gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
            self.gapless_trim_spec
        }

        fn estimated_remaining_frames(&self) -> Option<u64> {
            Some(self.remaining_frames)
        }

        fn next_block(&mut self, _out: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
            StageStatus::Eof
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

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

    fn new_runner(
        transforms: Vec<Box<dyn TransformStage>>,
        remaining_frames: u64,
        gapless_trim_spec: Option<GaplessTrimSpec>,
        supports_gapless_trim: bool,
    ) -> PipelineRunner {
        PipelineRunner::new(
            Box::new(TestSource),
            Box::new(TestDecoder::new(remaining_frames, gapless_trim_spec)),
            transforms,
            Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])),
            SinkLatencyConfig::default(),
            std::time::Duration::from_millis(50),
            false,
            supports_gapless_trim,
        )
        .expect("failed to construct test runner")
    }

    #[test]
    fn remaining_frames_hint_ignores_gapless_tail_without_gapless_stage() {
        let mut runner = new_runner(
            Vec::new(),
            3,
            Some(GaplessTrimSpec {
                head_frames: 0,
                tail_frames: 1,
            }),
            false,
        );
        let mut ctx = PipelineContext::default();
        runner
            .prepare_decode(&InputRef::TrackToken("track-a".to_string()), &mut ctx)
            .expect("prepare_decode failed");

        assert_eq!(runner.playable_remaining_frames_hint(), Some(3));
    }

    #[test]
    fn remaining_frames_hint_applies_gapless_tail_with_gapless_stage() {
        let mut runner = new_runner(
            Vec::new(),
            3,
            Some(GaplessTrimSpec {
                head_frames: 0,
                tail_frames: 1,
            }),
            true,
        );
        let mut ctx = PipelineContext::default();
        runner
            .prepare_decode(&InputRef::TrackToken("track-a".to_string()), &mut ctx)
            .expect("prepare_decode failed");

        assert_eq!(runner.playable_remaining_frames_hint(), Some(2));
    }
}
