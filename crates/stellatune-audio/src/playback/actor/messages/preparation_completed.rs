//! Returns request-backed preparation work to the owning actor.

use super::super::PlaybackActor;
use super::ControlResult;
use crate::playback::{
    event::{PlaybackEvent, PlaybackState},
    lifecycle::{publish_control_failure, set_state},
    pump::{activate, promote_or_end},
    state::{DrainPhase, NextTrack, PreparationPurpose, PreparationResult},
};
use lattice_actor::reply::ReplyTo;
use lattice_actor::{context::HandlerContext, error::ActorError, traits::Handler};
use stellatune_audio_core::error::FailureStage;
use stellatune_audio_core::error::PlaybackControlError;

/// Returns request-backed preparation work to the owning actor.
#[derive(lattice_actor::Message)]
pub(in crate::playback) struct PreparationCompleted {
    pub(in crate::playback::actor) prepared: PreparationResult,
    pub(in crate::playback::actor) reply_to: ReplyTo<ControlResult>,
}

impl Handler<PreparationCompleted> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: PreparationCompleted,
    ) -> Result<(), ActorError> {
        let PreparationCompleted { prepared, reply_to } = message;
        let matches = self
            .session
            .pending_preparation
            .iter()
            .chain(self.session.next.pending())
            .any(|pending| pending.id == prepared.id && pending.generation == prepared.generation);
        if !matches || prepared.generation != self.session.generation {
            let _ = reply_to.send(Err(PlaybackControlError::Closed));
            return Ok(());
        }
        if matches!(prepared.purpose, PreparationPurpose::Next) {
            self.session.next = NextTrack::Empty;
        } else {
            self.session.pending_preparation = None;
        }
        let mut state = *ctx.behavior();
        let response;
        match prepared.purpose {
            PreparationPurpose::Current => match prepared.result {
                Ok(prepared) => {
                    let item_id = prepared.plan.item.id;
                    match activate(
                        prepared,
                        &self.config,
                        self.session.output_gain,
                        &self.session.output_workers,
                    ) {
                        Ok(current) => {
                            if self.session.wants_playing {
                                let _ = current.output.resume();
                                set_state(&mut state, PlaybackState::Playing, &self.event_tx);
                            } else {
                                let _ = current.output.pause();
                                set_state(&mut state, PlaybackState::Ready, &self.event_tx);
                            }
                            self.session.current = Some(current);
                            let _ = self.event_tx.send(PlaybackEvent::TrackChanged { item_id });
                            response = Ok(());
                        },
                        Err(error) => {
                            set_state(&mut state, PlaybackState::Failed, &self.event_tx);
                            response = Err(error);
                        },
                    }
                },
                Err(error) => {
                    set_state(&mut state, PlaybackState::Failed, &self.event_tx);
                    publish_control_failure(&error, &self.event_tx);
                    response = Err(error);
                },
            },
            PreparationPurpose::Next => match prepared.result {
                Ok(prepared) => {
                    self.session.next.clear();
                    self.session.next = NextTrack::Ready(Box::new(prepared));
                    response = self.apply_advance(&mut state);
                    if state == PlaybackState::Buffering
                        && self
                            .session
                            .current
                            .as_ref()
                            .is_some_and(|current| current.drain_phase == DrainPhase::Complete)
                    {
                        if let Some(item_id) =
                            self.session.current.as_ref().map(|current| current.item_id)
                        {
                            let _ = self.event_tx.send(PlaybackEvent::Buffering {
                                item_id,
                                active: false,
                            });
                        }
                        promote_or_end(&mut self.session, &mut state, &self.config, &self.event_tx);
                    }
                },
                Err(error) => {
                    self.session.next.clear();
                    self.session.advance_options = None;
                    self.session.force_transition = false;
                    publish_control_failure(&error, &self.event_tx);
                    response = Err(error);
                    if state == PlaybackState::Buffering
                        && self
                            .session
                            .current
                            .as_ref()
                            .is_some_and(|current| current.drain_phase == DrainPhase::Complete)
                    {
                        promote_or_end(&mut self.session, &mut state, &self.config, &self.event_tx);
                    }
                },
            },
            PreparationPurpose::Recovery { .. } => {
                response = Err(PlaybackControlError::failed(
                    FailureStage::Runtime,
                    "recovery completed on request preparation path".to_owned(),
                ));
            },
        }
        self.transition(ctx, state);
        if response.is_ok() && prepared.purpose == PreparationPurpose::Current {
            if let Some(current) = self.session.current.as_ref() {
                current.output.reply_when_started(reply_to);
            }
        } else {
            let _ = reply_to.send(response);
        }
        Ok(())
    }
}
