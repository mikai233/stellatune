//! Applies output negotiation only to the session that requested it.
use super::super::PlaybackActor;
use super::{ControlResult, rebuild_output::PreparedOutput};
use crate::playback::{
    event::PlaybackState,
    lifecycle::{finish_seek, reject_pending, set_state, start_seek},
    sink_worker::SinkWorker,
    state::PendingSeek,
};
use lattice_actor::{context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Handler};
use std::sync::Arc;
use stellatune_audio_core::{
    decoder::DecoderSeekStatus,
    error::PlaybackControlError,
    playback::{MediaTime, PlaybackItemId},
};

#[derive(lattice_actor::Message)]
pub(in crate::playback) struct OutputPrepared {
    pub(in crate::playback::actor) id: u64,
    pub(in crate::playback::actor) generation: u64,
    pub(in crate::playback::actor) item_id: PlaybackItemId,
    pub(in crate::playback::actor) result: Result<PreparedOutput, PlaybackControlError>,
    pub(in crate::playback::actor) reply_to: ReplyTo<ControlResult>,
}
impl Handler<OutputPrepared> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: OutputPrepared,
    ) -> Result<(), ActorError> {
        let OutputPrepared {
            id,
            generation,
            item_id,
            result,
            reply_to,
        } = message;
        if id != self.session.output_rebuild_id
            || generation != self.session.generation
            || !self
                .session
                .current
                .as_ref()
                .is_some_and(|current| current.item_id == item_id)
        {
            let _ = reply_to.send(Err(PlaybackControlError::Closed));
            return Ok(());
        }
        let PreparedOutput {
            target,
            normalizer,
            output_format,
            transforms,
        } = match result {
            Ok(prepared) => prepared,
            Err(error) => {
                let _ = reply_to.send(Err(error));
                return Ok(());
            },
        };
        let current = self.session.current.as_ref().unwrap();
        // Include playback/seek progress made while negotiation was off-turn.
        let checkpoint = MediaTime::from_frames(
            current.consumed_position_frame(),
            current.pipeline.mix_format.sample_rate,
        );
        let output = SinkWorker::start(
            Arc::clone(&current.sink_factory),
            output_format,
            self.config.max_pcm_blocks,
            self.config.buffering,
            self.session.output_gain,
            &self.session.output_workers,
        );
        let mut output = match output {
            Ok(output) => output,
            Err(error) => {
                let _ = reply_to.send(Err(error));
                return Ok(());
            },
        };
        reject_pending(&mut self.session);
        let current = self.session.current.as_mut().unwrap();
        let old_rate = current.pipeline.mix_format.sample_rate;
        current.pipeline.duration_frames = current
            .pipeline
            .duration_frames
            .map(|frames| MediaTime::from_frames(frames, old_rate).to_frames(target.sample_rate));
        current.seek_fade_frames = MediaTime::from_frames(current.seek_fade_frames, old_rate)
            .to_frames(target.sample_rate);
        current.pipeline.normalizer = Some(normalizer);
        current.pipeline.mix_format = target;
        current.output_format = output_format;
        current.post_mix_transforms = transforms;
        std::mem::swap(&mut output, &mut current.output);
        output.shutdown();
        let mut state = *ctx.behavior();
        match start_seek(&mut self.session, checkpoint) {
            Ok((_, DecoderSeekStatus::Complete(result))) => {
                finish_seek(&mut self.session, result, &self.event_tx);
                let result = if self.session.wants_playing {
                    self.session.current.as_ref().unwrap().output.resume()
                } else {
                    Ok(())
                };
                set_state(
                    &mut state,
                    if self.session.wants_playing {
                        PlaybackState::Playing
                    } else {
                        PlaybackState::Paused
                    },
                    &self.event_tx,
                );
                if result.is_ok() {
                    self.session
                        .current
                        .as_ref()
                        .unwrap()
                        .output
                        .reply_when_started(reply_to);
                } else {
                    let _ = reply_to.send(result);
                }
            },
            Ok((item_id, DecoderSeekStatus::Pending)) => {
                if self.session.wants_playing {
                    let _ = self.session.current.as_ref().unwrap().output.resume();
                }
                set_state(&mut state, PlaybackState::Buffering, &self.event_tx);
                self.session.pending_seek = Some(PendingSeek {
                    response: reply_to,
                    item_id,
                });
            },
            Err(error) => {
                crate::playback::lifecycle::fail_current_error(
                    &mut self.session,
                    &mut state,
                    &self.event_tx,
                    error.clone(),
                );
                let _ = reply_to.send(Err(error));
            },
        }
        self.transition(ctx, state);
        Ok(())
    }
}
