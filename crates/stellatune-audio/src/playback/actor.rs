use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use stellatune_audio_core::{
    DecoderSeekStatus, MediaTime, PlaybackControlError, SourceCancellation, SourceOpenPurpose,
};
use tokio::sync::broadcast;

use crate::planner::{PipelinePlanner, PlaybackRequest};

use super::control::{Command, CommandKind, CommandReply, SwitchTransition};
use super::event::{PlaybackEvent, PlaybackRuntimeSnapshot, PlaybackState};
use super::lifecycle::{
    advance_pending_seek, fail_current, finish_seek, publish_control_failure, reject_pending,
    set_state, start_seek, stop_current,
};
use super::preparation::spawn_preparation;
use super::pump::{activate, promote_or_end, pump_once};
use super::runtime::PlaybackRuntimeConfig;
use super::sink_worker::SinkWorker;
use super::state::{ActorState, DrainPhase, PendingSeek, PreparationKind, PreparationResult};
use super::transition::configure_forced_transition;
pub(super) fn actor_loop(
    config: PlaybackRuntimeConfig,
    command_rx: Receiver<Command>,
    event_tx: broadcast::Sender<PlaybackEvent>,
) {
    // At most a small number of current/next/recovery preparations can be useful.
    // A bounded completion mailbox prevents stale slow opens from accumulating
    // unbounded results while the user switches repeatedly.
    let (preparation_tx, preparation_rx) =
        crossbeam_channel::bounded(config.command_capacity.max(1));
    let mut actor = ActorState {
        state: PlaybackState::Idle,
        generation: 0,
        preparation_cancellation: SourceCancellation::default(),
        current: None,
        next: None,
        next_preparing: false,
        pending_current_response: None,
        pending_next_response: None,
        pending_seek: None,
        crossfade: None,
        force_transition: false,
        policies: config.policies,
        output_gain: 1.0,
    };
    let planner = PipelinePlanner;
    let mut closed = false;

    while !closed {
        while let Ok(prepared) = preparation_rx.try_recv() {
            handle_prepared(prepared, &config, &preparation_tx, &event_tx, &mut actor);
        }

        match command_rx.recv_timeout(Duration::from_millis(2)) {
            Ok(command) => {
                closed = handle_command(
                    command,
                    &config,
                    &planner,
                    &preparation_tx,
                    &event_tx,
                    &mut actor,
                );
            },
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => closed = true,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {},
        }

        advance_pending_seek(&event_tx, &mut actor);

        if actor.state == PlaybackState::Playing && actor.pending_seek.is_none() {
            pump_once(&config, &preparation_tx, &event_tx, &mut actor);
        }
    }
    stop_current(&mut actor);
}

fn handle_command(
    command: Command,
    config: &PlaybackRuntimeConfig,
    planner: &PipelinePlanner,
    preparation_tx: &Sender<PreparationResult>,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut ActorState,
) -> bool {
    match command.kind {
        CommandKind::Switch { item, options } => {
            advance_generation(actor);
            reject_pending(actor);
            if actor.current.is_some()
                && options.transition == SwitchTransition::UseConfiguredPolicy
            {
                actor.next = None;
                actor.next_preparing = true;
                actor.crossfade = None;
                actor.force_transition = true;
                let plan = match planner.plan(
                    PlaybackRequest {
                        item,
                        policies: actor.policies,
                    },
                    &config.registry,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        let _ = command.response.send(Err(PlaybackControlError::failed(
                            "planner",
                            error.to_string(),
                        )));
                        return false;
                    },
                };
                spawn_preparation(
                    plan,
                    actor.generation,
                    SourceOpenPurpose::Prewarm,
                    PreparationKind::Next {
                        response: command.response,
                    },
                    preparation_tx.clone(),
                    actor.preparation_cancellation.clone(),
                );
                return false;
            }
            stop_current(actor);
            actor.next = None;
            actor.next_preparing = false;
            actor.crossfade = None;
            actor.force_transition = false;
            set_state(actor, PlaybackState::Preparing, event_tx);
            let request = PlaybackRequest {
                item,
                policies: actor.policies,
            };
            let plan = match planner.plan(request, &config.registry) {
                Ok(plan) => plan,
                Err(error) => {
                    set_state(actor, PlaybackState::Failed, event_tx);
                    let _ = command.response.send(Err(PlaybackControlError::failed(
                        "planner",
                        error.to_string(),
                    )));
                    return false;
                },
            };
            let generation = actor.generation;
            actor.pending_current_response = Some(command.response);
            spawn_preparation(
                plan,
                generation,
                SourceOpenPurpose::Initial,
                PreparationKind::Current {
                    autoplay: options.autoplay,
                    response: actor.pending_current_response.take().unwrap(),
                },
                preparation_tx.clone(),
                actor.preparation_cancellation.clone(),
            );
        },
        CommandKind::QueueNext { item } => {
            if actor.current.is_none() {
                let _ = command
                    .response
                    .send(Err(PlaybackControlError::InvalidState));
                return false;
            }
            let request = PlaybackRequest {
                item,
                policies: actor.policies,
            };
            let plan = match planner.plan(request, &config.registry) {
                Ok(plan) => plan,
                Err(error) => {
                    let _ = command.response.send(Err(PlaybackControlError::failed(
                        "planner",
                        error.to_string(),
                    )));
                    return false;
                },
            };
            actor.next = None;
            advance_generation(actor);
            actor.next_preparing = true;
            actor.force_transition = false;
            if let Some(response) = actor.pending_next_response.take() {
                let _ = response.send(Err(PlaybackControlError::Closed));
            }
            let generation = actor.generation;
            spawn_preparation(
                plan,
                generation,
                SourceOpenPurpose::Prewarm,
                PreparationKind::Next {
                    response: command.response,
                },
                preparation_tx.clone(),
                actor.preparation_cancellation.clone(),
            );
        },
        CommandKind::Play => {
            let result = match actor.current.as_mut() {
                Some(current) => current.output.resume().map(|_| CommandReply::Unit),
                None => Err(PlaybackControlError::InvalidState),
            };
            if result.is_ok() {
                set_state(actor, PlaybackState::Playing, event_tx);
            }
            let _ = command.response.send(result);
        },
        CommandKind::Pause => {
            let result = match actor.current.as_mut() {
                Some(current) => current.output.pause().map(|_| CommandReply::Unit),
                None => Err(PlaybackControlError::InvalidState),
            };
            if result.is_ok() {
                set_state(actor, PlaybackState::Paused, event_tx);
            }
            let _ = command.response.send(result);
        },
        CommandKind::Seek(position) => {
            if let Some(pending) = actor.pending_seek.take() {
                let _ = pending.response.send(Err(PlaybackControlError::Closed));
            }
            let resume_state = if actor.state == PlaybackState::Paused {
                PlaybackState::Paused
            } else {
                PlaybackState::Playing
            };
            match start_seek(actor, position) {
                Ok((_item_id, DecoderSeekStatus::Complete(result))) => {
                    finish_seek(actor, result, event_tx);
                    let _ = command.response.send(Ok(CommandReply::Unit));
                },
                Ok((item_id, DecoderSeekStatus::Pending)) => {
                    set_state(actor, PlaybackState::Buffering, event_tx);
                    let _ = event_tx.send(PlaybackEvent::Buffering {
                        item_id,
                        active: true,
                    });
                    actor.pending_seek = Some(PendingSeek {
                        response: command.response,
                        resume_state,
                        item_id,
                    });
                },
                Err(error) => {
                    let _ = command.response.send(Err(error));
                },
            }
        },
        CommandKind::Stop => {
            reject_pending(actor);
            actor.crossfade = None;
            actor.force_transition = false;
            stop_current(actor);
            actor.next = None;
            actor.next_preparing = false;
            set_state(actor, PlaybackState::Idle, event_tx);
            let _ = command.response.send(Ok(CommandReply::Unit));
        },
        CommandKind::SetOutputGain { gain, ramp } => {
            actor.output_gain = gain;
            let result = match actor.current.as_mut() {
                Some(current) => current
                    .output
                    .set_gain(gain, ramp.to_frames(current.output_format.sample_rate))
                    .map(|_| CommandReply::Unit),
                None => Ok(CommandReply::Unit),
            };
            let _ = command.response.send(result);
        },
        CommandKind::SetPolicies(policies) => {
            actor.policies = policies;
            let _ = command.response.send(Ok(CommandReply::Unit));
        },
        CommandKind::RebuildOutput => {
            let should_resume = actor.state == PlaybackState::Playing;
            let output_gain = actor.output_gain;
            let result = match actor.current.as_mut() {
                Some(current) => (|| {
                    current.output.shutdown();
                    current.output = SinkWorker::start(
                        Arc::clone(&current.sink_factory),
                        current.output_format,
                        config.pcm_ring_blocks,
                        output_gain,
                    )?;
                    if should_resume {
                        current.output.resume()?;
                    }
                    Ok(CommandReply::Unit)
                })(),
                None => Ok(CommandReply::Unit),
            };
            let _ = command.response.send(result);
        },
        CommandKind::Snapshot => {
            let current_item_id = actor.current.as_ref().map(|current| current.item_id);
            let consumed_position = actor
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
            let _ = command
                .response
                .send(Ok(CommandReply::Snapshot(PlaybackRuntimeSnapshot {
                    state: actor.state,
                    current_item_id,
                    consumed_position,
                })));
        },
        CommandKind::Shutdown => {
            actor.preparation_cancellation.cancel();
            reject_pending(actor);
            stop_current(actor);
            let _ = command.response.send(Ok(CommandReply::Unit));
            return true;
        },
    }
    false
}

pub(super) fn advance_generation(actor: &mut ActorState) {
    actor.preparation_cancellation.cancel();
    actor.preparation_cancellation = SourceCancellation::default();
    actor.generation = actor.generation.wrapping_add(1);
}

fn handle_prepared(
    prepared: PreparationResult,
    config: &PlaybackRuntimeConfig,
    preparation_tx: &Sender<PreparationResult>,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut ActorState,
) {
    if prepared.generation != actor.generation {
        match prepared.kind {
            PreparationKind::Current { response, .. } | PreparationKind::Next { response } => {
                let _ = response.send(Err(PlaybackControlError::Closed));
            },
            PreparationKind::Recovery { .. } => {},
        }
        return;
    }
    match prepared.kind {
        PreparationKind::Current { autoplay, response } => match prepared.result {
            Ok(prepared) => {
                let item_id = prepared.plan.item.id;
                match activate(prepared, config, actor.output_gain) {
                    Ok(current) => {
                        if autoplay {
                            let _ = current.output.resume();
                            set_state(actor, PlaybackState::Playing, event_tx);
                        } else {
                            let _ = current.output.pause();
                            set_state(actor, PlaybackState::Ready, event_tx);
                        }
                        actor.current = Some(current);
                        let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
                        let _ = response.send(Ok(CommandReply::Unit));
                    },
                    Err(error) => {
                        set_state(actor, PlaybackState::Failed, event_tx);
                        let _ = response.send(Err(error));
                    },
                }
            },
            Err(error) => {
                set_state(actor, PlaybackState::Failed, event_tx);
                publish_control_failure(&error, event_tx);
                let _ = response.send(Err(error));
            },
        },
        PreparationKind::Next { response } => match prepared.result {
            Ok(prepared) => {
                actor.next_preparing = false;
                actor.next = Some(prepared);
                configure_forced_transition(actor);
                let _ = response.send(Ok(CommandReply::Unit));
                if actor.state == PlaybackState::Buffering
                    && actor
                        .current
                        .as_ref()
                        .is_some_and(|current| current.drain_phase == DrainPhase::Complete)
                {
                    if let Some(item_id) = actor.current.as_ref().map(|current| current.item_id) {
                        let _ = event_tx.send(PlaybackEvent::Buffering {
                            item_id,
                            active: false,
                        });
                    }
                    promote_or_end(actor, config, event_tx);
                }
            },
            Err(error) => {
                actor.next_preparing = false;
                publish_control_failure(&error, event_tx);
                let _ = response.send(Err(error));
                if actor.state == PlaybackState::Buffering
                    && actor
                        .current
                        .as_ref()
                        .is_some_and(|current| current.drain_phase == DrainPhase::Complete)
                {
                    promote_or_end(actor, config, event_tx);
                }
            },
        },
        PreparationKind::Recovery {
            item_id,
            checkpoint,
            resume_state,
            attempt,
        } => match prepared
            .result
            .and_then(|prepared| activate(prepared, config, actor.output_gain))
        {
            Ok(mut recovered) if recovered.item_id == item_id => {
                if let Some(mut failed) = actor.current.take() {
                    failed.output.shutdown();
                }
                recovered.fade_in_start_frame = recovered.position_base_frame;
                recovered.fade_in_frames = recovered.seek_fade_frames;
                if resume_state == PlaybackState::Playing {
                    let _ = recovered.output.resume();
                } else {
                    let _ = recovered.output.pause();
                }
                actor.current = Some(recovered);
                set_state(actor, resume_state, event_tx);
            },
            Ok(_) => fail_current(
                actor,
                event_tx,
                "runtime",
                "recovery completed for the wrong playback item".to_owned(),
            ),
            Err(error) => {
                let retry_limit = actor.policies.max_recovery_attempts.max(1);
                if attempt < retry_limit
                    && actor
                        .current
                        .as_ref()
                        .is_some_and(|current| current.item_id == item_id)
                {
                    let plan = actor.current.as_ref().unwrap().recovery_plan.clone();
                    spawn_preparation(
                        plan,
                        actor.generation,
                        SourceOpenPurpose::Recovery,
                        PreparationKind::Recovery {
                            item_id,
                            checkpoint,
                            resume_state,
                            attempt: attempt + 1,
                        },
                        preparation_tx.clone(),
                        actor.preparation_cancellation.clone(),
                    );
                } else {
                    fail_current(actor, event_tx, "recovery", error.to_string());
                }
            },
        },
    }
}
