//! Shared successor matching and activation for navigation and preparation completion.

use super::{PlaybackActor, messages::ControlResult};
use crate::playback::{
    control::{SwitchOptions, SwitchTransition},
    event::{PlaybackEvent, PlaybackState},
    lifecycle::{reject_pending, set_state, stop_current},
    pump::activate,
    transition::configure_forced_transition,
};
use stellatune_audio_core::{error::PlaybackControlError, playback::PlaybackItemId};

impl PlaybackActor {
    pub(super) fn apply_overlap_intent(
        &self,
        state: &mut PlaybackState,
        options: SwitchOptions,
    ) -> ControlResult {
        let current = self
            .session
            .current
            .as_ref()
            .ok_or(PlaybackControlError::InvalidState)?;
        if options.autoplay && *state != PlaybackState::Playing {
            current.output.resume()?;
            set_state(state, PlaybackState::Playing, &self.event_tx);
        } else if !options.autoplay && *state != PlaybackState::Paused {
            current.output.pause()?;
            set_state(state, PlaybackState::Paused, &self.event_tx);
        }
        Ok(())
    }

    pub(super) fn matches_successor(&self, id: PlaybackItemId) -> bool {
        self.session.next.item_id() == Some(id)
            || self
                .session
                .crossfade
                .as_ref()
                .is_some_and(|fade| fade.next.item_id == id)
    }

    /// Applies a claimed successor once ready; an already-started overlap is retained.
    pub(super) fn apply_advance(&mut self, state: &mut PlaybackState) -> ControlResult {
        let Some(mut options) = self.session.advance_options else {
            return Ok(());
        };
        options.autoplay = self.session.wants_playing;
        if self.session.crossfade.is_some() {
            self.apply_overlap_intent(state, options)?;
            return Ok(());
        }
        if self.session.next.as_mut().is_none() {
            return Ok(());
        }
        self.session.advance_options = None;
        reject_pending(&mut self.session);
        if options.transition == SwitchTransition::UseConfiguredPolicy
            && options.autoplay
            && *state != PlaybackState::Recovering
            && self.session.current.is_some()
        {
            self.session.force_transition = true;
            configure_forced_transition(&mut self.session);
            self.session
                .current
                .as_ref()
                .expect("current checked")
                .output
                .resume()?;
            set_state(state, PlaybackState::Playing, &self.event_tx);
            return Ok(());
        }
        let next = self.session.next.take().expect("ready checked");
        if let Some(pending) = self.session.pending_preparation.take() {
            pending.cancellation.cancel();
        }
        if let Some(pending) = self.session.pending_recovery.take() {
            pending.cancellation.cancel();
        }
        stop_current(&mut self.session);
        self.session.force_transition = false;
        let mut current = match activate(
            next,
            &self.config,
            self.session.output_gain,
            &self.session.output_workers,
        ) {
            Ok(current) => current,
            Err(error) => {
                set_state(state, PlaybackState::Failed, &self.event_tx);
                return Err(error);
            },
        };
        current.fade_in_frames = current.seek_fade_frames;
        let item_id = current.item_id;
        if options.autoplay {
            current.output.resume()?;
        } else {
            current.output.pause()?;
        }
        self.session.current = Some(current);
        set_state(
            state,
            if options.autoplay {
                PlaybackState::Playing
            } else {
                PlaybackState::Ready
            },
            &self.event_tx,
        );
        let _ = self.event_tx.send(PlaybackEvent::TrackChanged { item_id });
        Ok(())
    }
}
