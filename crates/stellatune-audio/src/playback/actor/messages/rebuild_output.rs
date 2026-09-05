//! Recreates the active sink for the current route configuration.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::playback::{
    event::PlaybackState,
    lifecycle::{finish_seek, reject_pending, set_state, start_seek},
    sink_worker::SinkWorker,
    state::PendingSeek,
};
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use std::sync::Arc;
use stellatune_audio_core::{
    decoder::DecoderSeekStatus, error::PlaybackControlError, playback::MediaTime,
};

/// Recreates the active sink for the current route configuration.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(in crate::playback) struct RebuildOutput;

impl Responder<RebuildOutput> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: RebuildOutput,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let Some(current) = self.session.current.as_ref() else {
            let _ = reply_to.send(Ok(()));
            return Ok(());
        };
        if !current
            .recovery_plan
            .item
            .source
            .descriptor()
            .capabilities
            .byte_seekable
        {
            let _ = reply_to.send(Err(PlaybackControlError::Unsupported));
            return Ok(());
        }
        let checkpoint = MediaTime::from_frames(
            current.consumed_position_frame(),
            current.pipeline.mix_format.sample_rate,
        );
        let output = SinkWorker::start(
            Arc::clone(&current.sink_factory),
            current.output_format,
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
