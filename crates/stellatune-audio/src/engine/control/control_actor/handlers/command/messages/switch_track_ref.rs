use std::sync::atomic::Ordering;
use std::time::Instant;

use tracing::trace;

use crate::types::TrackRef;
use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};

use super::super::super::tick::ControlTickMessage;
use super::super::{
    DisruptFadeKind, Event, ManualSwitchTiming, PlayerState, SessionStopMode, emit_position_event,
    ensure_output_spec_prewarm, flush_pending_plugin_disables, maybe_fade_out_before_disrupt,
    next_position_session_id, set_state, stop_decode_session, track_ref_to_engine_token,
    track_ref_to_event_path,
};
use crate::engine::control::control_actor::ControlActor;

pub(crate) struct SwitchTrackRefMessage {
    pub(crate) track: TrackRef,
    pub(crate) lazy: bool,
}

impl Message for SwitchTrackRefMessage {
    type Response = Result<(), String>;
}

impl Handler<SwitchTrackRefMessage> for ControlActor {
    fn handle(
        &mut self,
        message: SwitchTrackRefMessage,
        ctx: &mut ActorContext<Self>,
    ) -> Result<(), String> {
        let (state, events, internal_tx, track_info) = (
            &mut self.state,
            &self.events,
            &self.internal_tx,
            &self.track_info,
        );
        let Some(path) = track_ref_to_engine_token(&message.track) else {
            let err = "track locator is empty".to_string();
            events.emit(Event::Error {
                message: err.clone(),
            });
            return Err(err);
        };
        let Some(event_path) = track_ref_to_event_path(&message.track) else {
            let err = "track locator is empty".to_string();
            events.emit(Event::Error {
                message: err.clone(),
            });
            return Err(err);
        };

        let switch_id = state.switch_timing_seq;
        state.switch_timing_seq = state.switch_timing_seq.saturating_add(1);
        state.manual_switch_timing = Some(ManualSwitchTiming {
            id: switch_id,
            from_track: state.current_track.clone(),
            to_track: event_path.clone(),
            began_at: Instant::now(),
            fade_done_at: None,
            stop_done_at: None,
            committed_at: None,
            play_requested_at: None,
            session_ready_at: None,
        });

        let buffered_samples = state
            .session
            .as_ref()
            .map(|s| s.buffered_samples.load(Ordering::Relaxed))
            .unwrap_or(0);
        let sink_pending_samples = state
            .output_sink_worker
            .as_ref()
            .map(|w| w.pending_samples())
            .unwrap_or(0);
        trace!(
            switch_id,
            from_track = state.current_track.as_deref().unwrap_or("<none>"),
            to_track = event_path.as_str(),
            player_state = ?state.player_state,
            wants_playback = state.wants_playback,
            seek_track_fade = state.seek_track_fade,
            buffered_samples,
            sink_pending_samples,
            "manual track switch(ref) begin"
        );

        maybe_fade_out_before_disrupt(state, DisruptFadeKind::TrackSwitch);
        if let Some(timing) = state.manual_switch_timing.as_mut() {
            timing.fade_done_at = Some(Instant::now());
        }
        stop_decode_session(state, track_info, SessionStopMode::KeepSink);
        if let Err(err_message) = flush_pending_plugin_disables(state, events) {
            events.emit(Event::Error {
                message: err_message,
            });
        }
        if let Some(timing) = state.manual_switch_timing.as_mut() {
            timing.stop_done_at = Some(Instant::now());
        }

        state.current_track = Some(path.clone());
        next_position_session_id(state);
        state.position_ms = 0;
        state.wants_playback = false;
        state.pending_session_start = false;
        state.play_request_started_at = None;
        state.seek_position_guard = None;
        track_info.store(None);
        events.emit(Event::TrackChanged {
            path: event_path.clone(),
        });
        emit_position_event(state, events);
        set_state(state, events, PlayerState::Stopped);

        if let Some(timing) = state.manual_switch_timing.as_mut() {
            timing.committed_at = Some(Instant::now());
        }
        trace!(
            switch_id,
            track = event_path.as_str(),
            "manual track switch(ref) committed"
        );

        if !message.lazy {
            let requested_at = Instant::now();
            if let Some(path) = state.current_track.as_deref()
                && let Some(timing) = state.manual_switch_timing.as_mut()
                && timing.to_track == path
                && timing.play_requested_at.is_none()
            {
                timing.play_requested_at = Some(requested_at);
            }
            state.wants_playback = true;
            state.play_request_started_at = Some(requested_at);
            state.pending_session_start = true;
            set_state(state, events, PlayerState::Buffering);
            ensure_output_spec_prewarm(state, internal_tx);
            let _ = ctx.actor_ref().cast(ControlTickMessage);
        }
        Ok(())
    }
}
