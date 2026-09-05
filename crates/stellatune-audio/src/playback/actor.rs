//! Lattice actor command admission and playback policy ownership.
//!
//! `PlaybackActor` is the single owner of [`PlaybackSession`]. Its Lattice
//! behavior is the externally observable [`PlaybackState`]; the session does
//! not duplicate that state. Typed requests carry control-plane data only.
//! Encoded bytes, decoded blocks, and sink writes remain outside the mailbox.
//!
//! Every source preparation carries both a monotonically wrapping generation
//! and a preparation identifier. Replacing a session or stopping advances its
//! generation. Successor preparation owns a separate cancellation token; claiming
//! it preserves both its identifier and generation. Completion
//! messages from older generations may complete their reply but must never
//! mutate the active session.
//!
//! Readiness notifications schedule bounded `PumpAudio` turns. Each turn yields
//! back to the mailbox; output demand determines whether more work is queued.
//!
//! [`messages`] gives each message its own payload and handler module. This
//! module retains the admission table, actor state, and runtime lifecycle.
//! [`preparation`] coordinates shared asynchronous work; [`navigation`] shares
//! successor matching and activation across commands and completion handlers.

use super::{
    event::{PlaybackEvent, PlaybackState},
    lifecycle::{reject_pending, stop_current},
    runtime::PlaybackRuntimeConfig,
    state::{NextTrack, PlaybackSession},
};
use crate::planner::PipelinePlanner;
use lattice_actor::{
    actor_behavior,
    context::{ActorContext, HandlerContext},
    error::{ActorError, ActorStopError},
    traits::{Actor, StopReason},
};
use std::time::Duration;
use tokio::sync::broadcast;

pub(super) mod messages;
mod navigation;
mod preparation;

use self::messages::{
    advance_to_next::AdvanceToNext, get_snapshot::GetSnapshot, pause::Pause, play::Play,
    preparation_completed::PreparationCompleted,
    preparation_deadline_elapsed::PreparationDeadlineElapsed, pump_audio::PumpAudio,
    rebuild_output::RebuildOutput, recovery_completed::RecoveryCompleted, seek::Seek,
    set_buffering::SetBuffering, set_next::SetNext, set_output_gain::SetOutputGain,
    set_policies::SetPolicies, stop_playback::StopPlayback, switch_to::SwitchTo,
};
use self::preparation::advance_generation;

actor_behavior! {
    PlaybackState {
        always => [
            SwitchTo,
            AdvanceToNext,
            SetNext,
            StopPlayback,
            SetOutputGain,
            SetPolicies,
            SetBuffering,
            RebuildOutput,
            GetSnapshot,
            PumpAudio,
            PreparationCompleted,
            PreparationDeadlineElapsed,
            RecoveryCompleted
        ];
        PlaybackState::Idle => [];
        PlaybackState::Preparing => [];
        PlaybackState::Ready => [Play, Pause, Seek];
        PlaybackState::Playing => [Play, Pause, Seek];
        PlaybackState::Paused => [Play, Pause, Seek];
        PlaybackState::Buffering => [Play, Pause, Seek];
        PlaybackState::Recovering => [Play, Pause, Seek];
        PlaybackState::Failed => [];
    }
}

/// Owns playback policy and serializes all changes to a [`PlaybackSession`].
pub(super) struct PlaybackActor {
    config: PlaybackRuntimeConfig,
    planner: PipelinePlanner,
    event_tx: broadcast::Sender<PlaybackEvent>,
    session: PlaybackSession,
}

impl PlaybackActor {
    /// Creates an idle actor from runtime configuration and its event channel.
    pub(super) fn new(
        config: PlaybackRuntimeConfig,
        event_tx: broadcast::Sender<PlaybackEvent>,
    ) -> Self {
        let policies = config.policies;
        Self {
            config,
            planner: PipelinePlanner,
            event_tx,
            session: PlaybackSession {
                output_workers: std::sync::Arc::default(),
                generation: 0,
                wants_playing: false,
                next_preparation_id: 0,
                pending_preparation: None,
                pending_recovery: None,
                current: None,
                next: NextTrack::Empty,
                advance_options: None,
                pending_seek: None,
                crossfade: None,
                force_transition: false,
                policies,
                output_gain: 1.0,
            },
        }
    }

    pub(super) fn output_workers(&self) -> std::sync::Arc<super::output_workers::OutputWorkers> {
        std::sync::Arc::clone(&self.session.output_workers)
    }

    fn transition(&self, ctx: &mut HandlerContext<'_, Self>, state: PlaybackState) {
        self.session.output_workers.pump.request();
        if *ctx.behavior() != state {
            ctx.transition_to(state);
        }
    }
}

impl Actor for PlaybackActor {
    type Error = ActorError;
    type Behavior = PlaybackState;

    async fn started(&mut self, ctx: &mut ActorContext<Self>) -> Result<(), ActorError> {
        let handle = ctx.self_handle();
        let pump = self.session.output_workers.pump.clone();
        ctx.spawn_scoped(async move {
            loop {
                pump.notified().await;
                if handle.tell(PumpAudio).await.is_err() {
                    break;
                }
            }
        });
        // Position reporting and compatibility with stages without readiness
        // notifications. This timer does not set the audio supply rate.
        let pump = self.session.output_workers.pump.clone();
        ctx.spawn_scoped(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                pump.request();
            }
        });
        Ok(())
    }

    async fn stopping(
        &mut self,
        _ctx: &mut ActorContext<Self>,
        _reason: StopReason,
    ) -> Result<(), ActorStopError> {
        advance_generation(&mut self.session);
        reject_pending(&mut self.session);
        stop_current(&mut self.session);
        if let Some(mut next) = self.session.next.take() {
            next.pipeline.decoder.reset();
            for transform in &mut next.pipeline.pre_mix_transforms {
                transform.reset();
            }
            for transform in &mut next.post_mix_transforms {
                transform.reset();
            }
            if let Some(normalizer) = next.pipeline.normalizer.as_mut() {
                normalizer.reset();
            }
        }
        if let Some(mut crossfade) = self.session.crossfade.take() {
            crossfade.next.pipeline.decoder.reset();
            for transform in &mut crossfade.next.pipeline.pre_mix_transforms {
                transform.reset();
            }
            if let Some(normalizer) = crossfade.next.pipeline.normalizer.as_mut() {
                normalizer.reset();
            }
        }
        Ok(())
    }
}
