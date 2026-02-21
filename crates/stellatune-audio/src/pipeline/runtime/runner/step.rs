//! Runner step and drain execution helpers.

use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;

use crate::pipeline::runtime::runner::{
    MAX_DRAIN_TAIL_ITERATIONS, MAX_PENDING_SINK_FLUSH_ATTEMPTS, PipelineRunner, RunnerState,
    StepResult,
};
use crate::pipeline::runtime::sink_session::SinkSession;
use crate::workers::sink::worker::SinkWriteError;

impl PipelineRunner {
    /// Executes one playback iteration and pushes at most one block to sink.
    ///
    /// The step synchronizes runtime control, handles pending seeks, runs
    /// decoder plus transform stages, and emits a [`StepResult`].
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
                let detail = self
                    .decoder
                    .runtime_error_detail()
                    .unwrap_or("decoder returned fatal status");
                return Err(PipelineError::StageFailure(format!(
                    "decoder fatal: {detail}"
                )));
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

    /// Flushes decoder and transform tails, then drains sink queued audio.
    pub(crate) fn drain(
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

    /// Attempts to push one block into sink session queue.
    ///
    /// On full queue, the block is retained as pending and the step reports idle.
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

    /// Flushes all queued pending blocks with bounded retries.
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

    /// Drains transform-generated tail audio after decoder flush.
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
}
