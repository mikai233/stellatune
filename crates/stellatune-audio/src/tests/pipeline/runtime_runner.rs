#![cfg(test)]

use crate::pipeline::assembly::StaticSinkPlan;
use crate::pipeline::runtime::runner::PipelineRunner;
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
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
