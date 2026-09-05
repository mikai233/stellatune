//! Requests actor state and sink-consumed position.

use super::super::PlaybackActor;
use crate::playback::event::PlaybackRuntimeSnapshot;
use lattice_actor::{
    context::HandlerContext, error::ActorError, reply::ReplyTo, traits::Responder,
};
use stellatune_audio_core::{error::PlaybackControlError, playback::MediaTime};
type SnapshotResult = Result<PlaybackRuntimeSnapshot, PlaybackControlError>;

/// Requests actor state and sink-consumed position.
#[derive(lattice_actor::Request)]
#[request(response = SnapshotResult)]
pub(in crate::playback) struct GetSnapshot;

impl Responder<GetSnapshot> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: GetSnapshot,
        reply_to: ReplyTo<SnapshotResult>,
    ) -> Result<(), ActorError> {
        let current_item_id = self.session.current.as_ref().map(|current| current.item_id);
        let consumed_position = self
            .session
            .current
            .as_ref()
            .map(|current| {
                MediaTime::from_frames(
                    current.position_base_frame.saturating_add(
                        current
                            .output
                            .clock()
                            .consumed_frames
                            .saturating_sub(current.sink_consumed_base_frame),
                    ),
                    current.mix_format.sample_rate,
                )
            })
            .unwrap_or(MediaTime::ZERO);
        let _ = reply_to.send(Ok(PlaybackRuntimeSnapshot {
            state: *ctx.behavior(),
            current_item_id,
            consumed_position,
        }));
        Ok(())
    }
}
