use std::time::{Duration, Instant};

use stellatune_audio_core::pipeline::context::TransitionTimePolicy;
use stellatune_audio_core::pipeline::context::{GainTransitionRequest, PipelineContext};
use stellatune_audio_core::pipeline::error::PipelineError;

use crate::runtime::runner::{PipelineRunner, RunnerState, StepResult};
use crate::runtime::sink_session::SinkSession;
use crate::runtime::transform::control::{TRANSITION_GAIN_STAGE_KEY, TransitionGainControl};
use crate::types::GainTransitionConfig;

const MAX_IDLE_STEPS: u32 = 2048;

fn build_request(
    config: GainTransitionConfig,
    target_gain: f32,
    ramp_ms: u32,
    available_frames_hint: Option<u64>,
    fade_out: bool,
) -> GainTransitionRequest {
    GainTransitionRequest {
        target_gain: target_gain.clamp(0.0, 1.0),
        ramp_ms,
        available_frames_hint,
        curve: config.curve,
        time_policy: if fade_out {
            config.fade_out_time_policy
        } else {
            config.fade_in_time_policy
        },
    }
}

pub(crate) fn request_fade_in_with_runner(
    runner: &mut PipelineRunner,
    ctx: &mut PipelineContext,
    config: GainTransitionConfig,
    ramp_ms: u32,
) -> Result<(), PipelineError> {
    request_fade_in_with_runner_inner(runner, ctx, config, ramp_ms, false)
}

pub(crate) fn request_fade_in_from_silence_with_runner(
    runner: &mut PipelineRunner,
    ctx: &mut PipelineContext,
    config: GainTransitionConfig,
    ramp_ms: u32,
) -> Result<(), PipelineError> {
    request_fade_in_with_runner_inner(runner, ctx, config, ramp_ms, true)
}

fn request_fade_in_with_runner_inner(
    runner: &mut PipelineRunner,
    ctx: &mut PipelineContext,
    config: GainTransitionConfig,
    ramp_ms: u32,
    from_silence: bool,
) -> Result<(), PipelineError> {
    if !runner.supports_transition_gain() {
        return Ok(());
    }

    if from_silence {
        let prime_request = GainTransitionRequest {
            target_gain: 0.0,
            ramp_ms: 0,
            available_frames_hint: None,
            curve: config.curve,
            time_policy: TransitionTimePolicy::Exact,
        };
        let prime_control = TransitionGainControl::new(prime_request);
        let handled =
            runner.apply_transform_control_to(TRANSITION_GAIN_STAGE_KEY, &prime_control, ctx)?;
        if !handled {
            return Err(PipelineError::StageFailure(
                "transition gain stage missing for fade-in prime request".to_string(),
            ));
        }
    }

    if ramp_ms == 0 {
        return Ok(());
    }

    let request = build_request(config, 1.0, ramp_ms, None, false);
    let control = TransitionGainControl::new(request);
    let handled = runner.apply_transform_control_to(TRANSITION_GAIN_STAGE_KEY, &control, ctx)?;
    if !handled {
        return Err(PipelineError::StageFailure(
            "transition gain stage missing for fade-in request".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn run_interrupt_fade_out(
    runner: &mut PipelineRunner,
    sink_session: &mut SinkSession,
    ctx: &mut PipelineContext,
    config: GainTransitionConfig,
    ramp_ms: u32,
    available_frames_hint: Option<u64>,
) -> Result<(), PipelineError> {
    if !runner.supports_transition_gain() {
        return Ok(());
    }

    if ramp_ms == 0 || runner.state() != RunnerState::Playing {
        return Ok(());
    }

    let request = build_request(config, 0.0, ramp_ms, available_frames_hint, true);
    let control = TransitionGainControl::new(request);
    let handled = runner.apply_transform_control_to(TRANSITION_GAIN_STAGE_KEY, &control, ctx)?;
    if !handled {
        return Err(PipelineError::StageFailure(
            "transition gain stage missing for fade-out request".to_string(),
        ));
    }

    let target_frames = expected_transition_frames(
        runner.output_sample_rate(),
        request.ramp_ms,
        request.time_policy,
        request.available_frames_hint,
    );
    if target_frames == 0 {
        return Ok(());
    }

    let deadline = Instant::now()
        + Duration::from_millis(ramp_ms as u64 + config.interrupt_max_extra_wait_ms as u64);
    let mut idle_steps = 0_u32;
    let mut produced_frames = 0_u64;
    while produced_frames < target_frames {
        if Instant::now() >= deadline {
            break;
        }
        match runner.step(sink_session, ctx)? {
            StepResult::Produced { frames } => {
                produced_frames = produced_frames.saturating_add(frames as u64);
                idle_steps = 0;
            },
            StepResult::Idle => {
                idle_steps = idle_steps.saturating_add(1);
                if idle_steps >= MAX_IDLE_STEPS {
                    break;
                }
                std::thread::yield_now();
            },
            StepResult::Eof => break,
        }
    }
    Ok(())
}

fn expected_transition_frames(
    sample_rate: Option<u32>,
    ramp_ms: u32,
    time_policy: TransitionTimePolicy,
    available_frames_hint: Option<u64>,
) -> u64 {
    if ramp_ms == 0 {
        return 0;
    }
    let sample_rate = sample_rate.unwrap_or(1).max(1) as u64;
    let nominal_frames = sample_rate
        .saturating_mul(ramp_ms as u64)
        .div_ceil(1000)
        .max(1);
    match time_policy {
        TransitionTimePolicy::Exact => nominal_frames,
        TransitionTimePolicy::FitToAvailable => available_frames_hint
            .map(|frames| frames.min(nominal_frames))
            .unwrap_or(nominal_frames),
    }
}

#[cfg(test)]
mod tests {
    use super::run_interrupt_fade_out;
    use crate::assembly::StaticSinkPlan;
    use crate::runtime::runner::PipelineRunner;
    use crate::runtime::sink_session::SinkSession;
    use crate::types::{GainTransitionConfig, SinkLatencyConfig};
    use stellatune_audio_core::pipeline::context::{
        AudioBlock, InputRef, PipelineContext, SourceHandle, StreamSpec,
    };
    use stellatune_audio_core::pipeline::error::PipelineError;
    use stellatune_audio_core::pipeline::stages::StageStatus;
    use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
    use stellatune_audio_core::pipeline::stages::sink::SinkStage;
    use stellatune_audio_core::pipeline::stages::source::SourceStage;

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

    #[derive(Default)]
    struct TestDecoder;

    impl DecoderStage for TestDecoder {
        fn prepare(
            &mut self,
            _source: &SourceHandle,
            _ctx: &mut PipelineContext,
        ) -> Result<StreamSpec, PipelineError> {
            Ok(StreamSpec {
                sample_rate: 48_000,
                channels: 2,
            })
        }

        fn sync_runtime_control(
            &mut self,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
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

    fn make_runner_without_transition_stage() -> PipelineRunner {
        PipelineRunner::new(
            Box::new(TestSource),
            Box::new(TestDecoder),
            Vec::new(),
            Box::new(StaticSinkPlan::new(vec![Box::new(TestSink)])),
            SinkLatencyConfig::default(),
            std::time::Duration::from_millis(50),
            false,
            false,
        )
        .expect("failed to construct test runner")
    }

    #[test]
    fn interrupt_fade_out_degrades_when_transition_stage_is_absent() {
        let mut runner = make_runner_without_transition_stage();
        let mut ctx = PipelineContext::default();
        let mut sink_session = SinkSession::new(
            SinkLatencyConfig::default(),
            std::time::Duration::from_millis(50),
        );

        run_interrupt_fade_out(
            &mut runner,
            &mut sink_session,
            &mut ctx,
            GainTransitionConfig::default(),
            48,
            Some(32),
        )
        .expect("fade out should degrade instead of failing");
    }
}
