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
//! A periodic `PumpAudio` message advances at most one PCM block. Saturated
//! tick messages may be dropped because the interval schedules another tick;
//! user commands therefore retain bounded latency under continuous playback.

use std::sync::Arc;
use std::time::{Duration, Instant};

use lattice_actor::{
    actor_behavior,
    context::{ActorContext, HandlerContext},
    error::{ActorError, ActorStopError, ActorTellError},
    reply::ReplyTo,
    traits::{Actor, ActorLifecycleState, Handler, Responder, StopReason},
};
use stellatune_audio_core::{
    decoder::DecoderSeekStatus,
    error::PlaybackControlError,
    playback::{MediaTime, PlaybackItemId},
    source::{SourceCancellation, SourceOpenPurpose},
};
use tokio::sync::broadcast;

use crate::planner::{PipelinePlanner, PlaybackPolicies};

use super::event::{PlaybackEvent, PlaybackRuntimeSnapshot, PlaybackState};
use super::lifecycle::{
    advance_pending_seek, fail_current, finish_seek, publish_control_failure, reject_pending,
    set_state, start_seek, stop_current,
};
use super::navigation::{AdvanceToNext, SetNext, SwitchTo};
use super::preparation::prepare_off_turn;
use super::pump::{activate, promote_or_end, pump_once};
use super::runtime::PlaybackRuntimeConfig;
use super::sink_worker::SinkWorker;
use super::state::{
    DrainPhase, NextTrack, PendingPreparation, PendingSeek, PlaybackSession, PreparationPurpose,
    PreparationResult, RecoveryPreparation,
};

type ControlResult = Result<(), PlaybackControlError>;
type SnapshotResult = Result<PlaybackRuntimeSnapshot, PlaybackControlError>;

/// Requests output playback or resumption.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct Play;

/// Requests output pause without discarding PCM.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct Pause;

/// Requests an absolute media seek on the current item.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct Seek {
    pub(super) position: MediaTime,
}

/// Requests teardown of the current playback session state.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct StopPlayback;

/// Requests a bounded final-output gain ramp.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct SetOutputGain {
    pub(super) gain: f32,
    pub(super) ramp: MediaTime,
}

/// Replaces policies captured by future executable plans.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct SetPolicies {
    pub(super) policies: PlaybackPolicies,
}

/// Recreates the active sink for the current route configuration.
#[derive(lattice_actor::Request)]
#[request(response = ControlResult)]
pub(super) struct RebuildOutput;

/// Requests actor state and sink-consumed position.
#[derive(lattice_actor::Request)]
#[request(response = SnapshotResult)]
pub(super) struct GetSnapshot;

/// Advances one bounded playback data-plane turn.
#[derive(lattice_actor::Message)]
pub(super) struct PumpAudio;

/// Returns request-backed preparation work to the owning actor.
#[derive(lattice_actor::Message)]
pub(super) struct PreparationCompleted {
    prepared: PreparationResult,
    reply_to: ReplyTo<ControlResult>,
}

/// Cancels preparation that is still current when its deadline expires.
#[derive(lattice_actor::Message)]
pub(super) struct PreparationDeadlineElapsed {
    id: u64,
    generation: u64,
}

/// Returns recovery preparation that has no external request reply.
#[derive(lattice_actor::Message)]
pub(super) struct RecoveryCompleted {
    prepared: PreparationResult,
}

actor_behavior! {
    PlaybackState {
        always => [
            SwitchTo,
            AdvanceToNext,
            SetNext,
            StopPlayback,
            SetOutputGain,
            SetPolicies,
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
    pub(super) config: PlaybackRuntimeConfig,
    pub(super) planner: PipelinePlanner,
    pub(super) event_tx: broadcast::Sender<PlaybackEvent>,
    pub(super) session: PlaybackSession,
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
                generation: 0,
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

    pub(super) fn transition(&self, ctx: &mut HandlerContext<'_, Self>, state: PlaybackState) {
        if *ctx.behavior() != state {
            ctx.transition_to(state);
        }
    }

    fn begin_preparation(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        purpose: PreparationPurpose,
        item_id: PlaybackItemId,
    ) -> (u64, u64, SourceCancellation, Instant) {
        self.session.next_preparation_id = self.session.next_preparation_id.wrapping_add(1);
        let id = self.session.next_preparation_id;
        let generation = self.session.generation;
        let timeout = self.config.command_timeouts.preparation;
        let deadline = Instant::now() + timeout;
        let cancellation = SourceCancellation::default();
        let pending = PendingPreparation {
            item_id,
            cancellation: cancellation.clone(),
            id,
            generation,
            purpose,
            deadline,
        };
        if matches!(purpose, PreparationPurpose::Next) {
            self.session.next = NextTrack::Preparing(pending);
        } else {
            self.session.pending_preparation = Some(pending);
        }
        ctx.notify_after(timeout, PreparationDeadlineElapsed { id, generation });
        (id, generation, cancellation, deadline)
    }

    pub(super) fn defer_preparation(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        plan: crate::planner::ExecutablePlaybackPlan,
        source_purpose: SourceOpenPurpose,
        purpose: PreparationPurpose,
        reply_to: ReplyTo<ControlResult>,
    ) {
        let (id, generation, cancellation, deadline) =
            self.begin_preparation(ctx, purpose, plan.item.id);
        if ctx
            .defer_reply(
                reply_to,
                prepare_off_turn(
                    plan,
                    id,
                    generation,
                    source_purpose,
                    purpose,
                    cancellation,
                    deadline,
                ),
                |prepared, reply_to| PreparationCompleted { prepared, reply_to },
            )
            .is_err()
        {
            self.cancel_preparation(purpose);
            let mut state = *ctx.behavior();
            match purpose {
                PreparationPurpose::Current { .. } => {
                    set_state(&mut state, PlaybackState::Failed, &self.event_tx);
                },
                PreparationPurpose::Next => self.session.next.clear(),
                PreparationPurpose::Recovery { .. } => {},
            }
            self.transition(ctx, state);
        }
    }

    fn launch_pending_recovery(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        state: &mut PlaybackState,
    ) {
        let Some(recovery) = self.session.pending_recovery.take() else {
            return;
        };
        let timeout = self.config.command_timeouts.preparation;
        let deadline = Instant::now() + timeout;
        self.session.pending_preparation = Some(PendingPreparation {
            item_id: recovery.plan.item.id,
            cancellation: recovery.cancellation.clone(),
            id: recovery.id,
            generation: recovery.generation,
            purpose: recovery.purpose,
            deadline,
        });
        ctx.notify_after(
            timeout,
            PreparationDeadlineElapsed {
                id: recovery.id,
                generation: recovery.generation,
            },
        );
        let task = prepare_off_turn(
            recovery.plan,
            recovery.id,
            recovery.generation,
            SourceOpenPurpose::Recovery,
            recovery.purpose,
            recovery.cancellation,
            deadline,
        );
        if ctx
            .pipe_to_self(task, |prepared| RecoveryCompleted { prepared })
            .is_err()
        {
            self.session.pending_preparation = None;
            fail_current(
                &mut self.session,
                state,
                &self.event_tx,
                "runtime",
                "playback preparation capacity is exhausted".to_owned(),
            );
        }
    }

    fn audit_preparation_deadline(&mut self, ctx: &mut HandlerContext<'_, Self>) {
        let expired: Vec<_> = self
            .session
            .pending_preparation
            .iter()
            .chain(self.session.next.pending())
            .filter(|pending| Instant::now() >= pending.deadline)
            .map(|pending| PreparationDeadlineElapsed {
                id: pending.id,
                generation: pending.generation,
            })
            .collect();
        for message in expired {
            self.handle_preparation_timeout(ctx, message);
        }
    }

    fn handle_preparation_timeout(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: PreparationDeadlineElapsed,
    ) {
        let Some(pending) = self
            .session
            .pending_preparation
            .iter()
            .chain(self.session.next.pending())
            .find(|pending| pending.id == message.id && pending.generation == message.generation)
        else {
            return;
        };
        let purpose = pending.purpose;
        self.cancel_preparation(purpose);
        let mut state = *ctx.behavior();
        match purpose {
            PreparationPurpose::Current { .. } => {
                set_state(&mut state, PlaybackState::Failed, &self.event_tx);
            },
            PreparationPurpose::Next => {
                self.session.next.clear();
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
            PreparationPurpose::Recovery { .. } => {
                let PreparationPurpose::Recovery {
                    item_id, attempt, ..
                } = purpose
                else {
                    unreachable!()
                };
                let retry_limit = self.session.policies.max_recovery_attempts.max(1);
                if attempt < retry_limit
                    && self
                        .session
                        .current
                        .as_ref()
                        .is_some_and(|current| current.item_id == item_id)
                {
                    self.schedule_recovery_retry(purpose);
                    self.launch_pending_recovery(ctx, &mut state);
                } else {
                    fail_current(
                        &mut self.session,
                        &mut state,
                        &self.event_tx,
                        "recovery",
                        "recovery preparation timed out".to_owned(),
                    );
                }
            },
        }
        self.transition(ctx, state);
    }

    fn cancel_preparation(&mut self, purpose: PreparationPurpose) {
        if matches!(purpose, PreparationPurpose::Next) {
            self.session.next.clear();
            self.session.advance_options = None;
            self.session.force_transition = false;
        } else if let Some(pending) = self.session.pending_preparation.take() {
            pending.cancellation.cancel();
        }
    }

    fn schedule_recovery_retry(&mut self, purpose: PreparationPurpose) {
        let PreparationPurpose::Recovery {
            item_id,
            checkpoint,
            resume_state,
            attempt,
        } = purpose
        else {
            return;
        };
        let Some(current) = self.session.current.as_ref() else {
            return;
        };
        self.session.next_preparation_id = self.session.next_preparation_id.wrapping_add(1);
        self.session.pending_recovery = Some(RecoveryPreparation {
            plan: current.recovery_plan.clone(),
            id: self.session.next_preparation_id,
            generation: self.session.generation,
            purpose: PreparationPurpose::Recovery {
                item_id,
                checkpoint,
                resume_state,
                attempt: attempt + 1,
            },
            cancellation: SourceCancellation::default(),
        });
    }
}

impl Actor for PlaybackActor {
    type Error = ActorError;
    type Behavior = PlaybackState;

    async fn started(&mut self, ctx: &mut ActorContext<Self>) -> Result<(), ActorError> {
        // `started` runs before the handle advertises Running. Delay the first
        // self-message so the interval cannot retire on Starting lifecycle admission.
        let handle = ctx.self_handle();
        ctx.spawn_scoped(async move {
            let interval = Duration::from_millis(2);
            loop {
                tokio::time::sleep(interval).await;
                match handle.try_tell(PumpAudio) {
                    Ok(()) | Err(ActorTellError::MailboxFull(_)) => {},
                    Err(ActorTellError::LifecycleUnavailable {
                        state: ActorLifecycleState::Starting,
                        ..
                    }) => {},
                    Err(
                        ActorTellError::MailboxClosed(_)
                        | ActorTellError::LifecycleUnavailable { .. },
                    ) => break,
                }
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
            next.decoder.reset();
            for transform in &mut next.pre_mix_transforms {
                transform.reset();
            }
            for transform in &mut next.post_mix_transforms {
                transform.reset();
            }
            if let Some(normalizer) = next.normalizer.as_mut() {
                normalizer.reset();
            }
        }
        if let Some(mut crossfade) = self.session.crossfade.take() {
            crossfade.next.decoder.reset();
            for transform in &mut crossfade.next.pre_mix_transforms {
                transform.reset();
            }
            if let Some(normalizer) = crossfade.next.normalizer.as_mut() {
                normalizer.reset();
            }
        }
        Ok(())
    }
}

impl Responder<Play> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: Play,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let result = self
            .session
            .current
            .as_mut()
            .ok_or(PlaybackControlError::InvalidState)
            .and_then(|current| current.output.resume());
        if result.is_ok() {
            if let Some(options) = self.session.advance_options.as_mut() {
                options.autoplay = true;
            }
            let mut state = *ctx.behavior();
            set_state(&mut state, PlaybackState::Playing, &self.event_tx);
            self.transition(ctx, state);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<Pause> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: Pause,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let result = self
            .session
            .current
            .as_mut()
            .ok_or(PlaybackControlError::InvalidState)
            .and_then(|current| current.output.pause());
        if result.is_ok() {
            if let Some(options) = self.session.advance_options.as_mut() {
                options.autoplay = false;
            }
            let mut state = *ctx.behavior();
            set_state(&mut state, PlaybackState::Paused, &self.event_tx);
            self.transition(ctx, state);
        }
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<Seek> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        request: Seek,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        if let Some(pending) = self.session.pending_seek.take() {
            let _ = pending.response.send(Err(PlaybackControlError::Closed));
        }
        let mut state = *ctx.behavior();
        let resume_state = if state == PlaybackState::Paused {
            PlaybackState::Paused
        } else {
            PlaybackState::Playing
        };
        match start_seek(&mut self.session, request.position) {
            Ok((_item_id, DecoderSeekStatus::Complete(result))) => {
                finish_seek(&mut self.session, result, &self.event_tx);
                let _ = reply_to.send(Ok(()));
            },
            Ok((item_id, DecoderSeekStatus::Pending)) => {
                set_state(&mut state, PlaybackState::Buffering, &self.event_tx);
                let _ = self.event_tx.send(PlaybackEvent::Buffering {
                    item_id,
                    active: true,
                });
                self.session.pending_seek = Some(PendingSeek {
                    response: reply_to,
                    resume_state,
                    item_id,
                });
            },
            Err(error) => {
                let _ = reply_to.send(Err(error));
            },
        }
        self.transition(ctx, state);
        Ok(())
    }
}

impl Responder<StopPlayback> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: StopPlayback,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        advance_generation(&mut self.session);
        reject_pending(&mut self.session);
        self.session.crossfade = None;
        self.session.force_transition = false;
        stop_current(&mut self.session);
        self.session.next.clear();
        self.session.next.clear();
        let mut state = *ctx.behavior();
        set_state(&mut state, PlaybackState::Idle, &self.event_tx);
        self.transition(ctx, state);
        let _ = reply_to.send(Ok(()));
        Ok(())
    }
}

impl Responder<SetOutputGain> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        request: SetOutputGain,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        self.session.output_gain = request.gain;
        let result = match self.session.current.as_mut() {
            Some(current) => current.output.set_gain(
                request.gain,
                request.ramp.to_frames(current.output_format.sample_rate),
            ),
            None => Ok(()),
        };
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<SetPolicies> for PlaybackActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        request: SetPolicies,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        self.session.policies = request.policies;
        let _ = reply_to.send(Ok(()));
        Ok(())
    }
}

impl Responder<RebuildOutput> for PlaybackActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _request: RebuildOutput,
        reply_to: ReplyTo<ControlResult>,
    ) -> Result<(), ActorError> {
        let should_resume = *ctx.behavior() == PlaybackState::Playing;
        let output_gain = self.session.output_gain;
        let result = match self.session.current.as_mut() {
            Some(current) => (|| {
                current.output.shutdown();
                current.output = SinkWorker::start(
                    Arc::clone(&current.sink_factory),
                    current.output_format,
                    self.config.pcm_ring_blocks,
                    output_gain,
                )?;
                if should_resume {
                    current.output.resume()?;
                }
                Ok(())
            })(),
            None => Ok(()),
        };
        let _ = reply_to.send(result);
        Ok(())
    }
}

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

impl Handler<PumpAudio> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: PumpAudio,
    ) -> Result<(), ActorError> {
        self.audit_preparation_deadline(ctx);
        let mut state = *ctx.behavior();
        advance_pending_seek(&self.event_tx, &mut self.session, &mut state);
        if matches!(state, PlaybackState::Playing | PlaybackState::Buffering)
            && self.session.pending_seek.is_none()
        {
            pump_once(&self.config, &self.event_tx, &mut self.session, &mut state);
        }
        self.launch_pending_recovery(ctx, &mut state);
        if let Err(error) = self.apply_advance(&mut state) {
            publish_control_failure(&error, &self.event_tx);
        }
        self.transition(ctx, state);
        Ok(())
    }
}

impl Handler<PreparationDeadlineElapsed> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: PreparationDeadlineElapsed,
    ) -> Result<(), ActorError> {
        self.handle_preparation_timeout(ctx, message);
        Ok(())
    }
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
            PreparationPurpose::Current { autoplay } => match prepared.result {
                Ok(prepared) => {
                    let item_id = prepared.plan.item.id;
                    match activate(prepared, &self.config, self.session.output_gain) {
                        Ok(current) => {
                            if autoplay {
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
                    "runtime",
                    "recovery completed on request preparation path".to_owned(),
                ));
            },
        }
        self.transition(ctx, state);
        let _ = reply_to.send(response);
        Ok(())
    }
}

impl Handler<RecoveryCompleted> for PlaybackActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: RecoveryCompleted,
    ) -> Result<(), ActorError> {
        let prepared = message.prepared;
        let matches = self
            .session
            .pending_preparation
            .as_ref()
            .is_some_and(|pending| {
                pending.id == prepared.id && pending.generation == prepared.generation
            });
        if !matches || prepared.generation != self.session.generation {
            return Ok(());
        }
        self.session.pending_preparation = None;
        let purpose = prepared.purpose;
        let PreparationPurpose::Recovery {
            item_id,
            checkpoint: _,
            resume_state,
            attempt,
        } = purpose
        else {
            return Ok(());
        };
        let mut state = *ctx.behavior();
        match prepared
            .result
            .and_then(|prepared| activate(prepared, &self.config, self.session.output_gain))
        {
            Ok(mut recovered) if recovered.item_id == item_id => {
                if let Some(mut failed) = self.session.current.take() {
                    failed.output.shutdown();
                }
                recovered.fade_in_start_frame = recovered.position_base_frame;
                recovered.fade_in_frames = recovered.seek_fade_frames;
                if resume_state == PlaybackState::Playing {
                    let _ = recovered.output.resume();
                } else {
                    let _ = recovered.output.pause();
                }
                self.session.current = Some(recovered);
                set_state(&mut state, resume_state, &self.event_tx);
            },
            Ok(_) => fail_current(
                &mut self.session,
                &mut state,
                &self.event_tx,
                "runtime",
                "recovery completed for the wrong playback item".to_owned(),
            ),
            Err(error) => {
                let retry_limit = self.session.policies.max_recovery_attempts.max(1);
                if attempt < retry_limit
                    && self
                        .session
                        .current
                        .as_ref()
                        .is_some_and(|current| current.item_id == item_id)
                {
                    self.schedule_recovery_retry(purpose);
                } else {
                    fail_current(
                        &mut self.session,
                        &mut state,
                        &self.event_tx,
                        "recovery",
                        error.to_string(),
                    );
                }
            },
        }
        self.launch_pending_recovery(ctx, &mut state);
        self.transition(ctx, state);
        Ok(())
    }
}

/// Invalidates outstanding preparation and advances the session generation.
pub(super) fn advance_generation(session: &mut PlaybackSession) {
    if let Some(pending) = session.pending_preparation.take() {
        pending.cancellation.cancel();
    }
    if let Some(pending) = session.pending_recovery.take() {
        pending.cancellation.cancel();
    }
    session.next.clear();
    session.advance_options = None;
    session.generation = session.generation.wrapping_add(1);
}
