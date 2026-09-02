use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use audioadapter_buffers::direct::InterleavedSlice;
use crossbeam_channel::{Receiver, Sender, TrySendError};
use rubato::{
    Async, FixedAsync, Indexing, Resampler, Resizable, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use stellatune_audio_core::{
    AudioBlock, AudioFormat, DecodeStatus, DecoderSeekStatus, DecoderStage, MediaTime,
    PlaybackControlError, PlaybackFailure, PlaybackItem, PlaybackItemId, SeekResult,
    SinkClockSnapshot, SinkFactory, SinkStage, SinkWriteState, SourceCancellation,
    SourceOpenPurpose, SourceOpenRequest, TransformPlacement, TransformStage, TransformStatus,
};
use tokio::sync::{broadcast, oneshot};

use crate::planner::{
    CrossfadeCurve, ExecutablePlaybackPlan, GainCurve, PipelinePlanner, PlaybackPolicies,
    PlaybackRequest, StageRegistrySnapshot, TransitionPolicy, can_fallback,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Idle,
    Preparing,
    Recovering,
    Ready,
    Playing,
    Paused,
    Buffering,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchTransition {
    UseConfiguredPolicy,
    ImmediateWithDeClick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitchOptions {
    pub autoplay: bool,
    pub transition: SwitchTransition,
}

impl Default for SwitchOptions {
    fn default() -> Self {
        Self {
            autoplay: true,
            transition: SwitchTransition::UseConfiguredPolicy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackRuntimeSnapshot {
    pub state: PlaybackState,
    pub current_item_id: Option<PlaybackItemId>,
    pub consumed_position: MediaTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaybackEvent {
    StateChanged(PlaybackState),
    TrackChanged {
        item_id: PlaybackItemId,
    },
    PlaybackEnded {
        item_id: PlaybackItemId,
    },
    Position {
        item_id: PlaybackItemId,
        position: MediaTime,
    },
    Buffering {
        item_id: PlaybackItemId,
        active: bool,
    },
    Failed(PlaybackFailure),
}

pub struct PlaybackRuntimeConfig {
    pub registry: StageRegistrySnapshot,
    pub policies: PlaybackPolicies,
    pub command_capacity: usize,
    pub pcm_ring_blocks: usize,
    pub block_frames: usize,
    pub event_capacity: usize,
}

impl PlaybackRuntimeConfig {
    pub fn new(registry: StageRegistrySnapshot) -> Self {
        Self {
            registry,
            policies: PlaybackPolicies::default(),
            command_capacity: 64,
            pcm_ring_blocks: 8,
            block_frames: 1024,
            event_capacity: 128,
        }
    }
}

pub struct PlaybackRuntime {
    controller: PlaybackController,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl PlaybackRuntime {
    pub fn start(config: PlaybackRuntimeConfig) -> Result<Self, PlaybackControlError> {
        let (command_tx, command_rx) = crossbeam_channel::bounded(config.command_capacity.max(1));
        let (event_tx, _) = broadcast::channel(config.event_capacity.max(1));
        let controller = PlaybackController {
            command_tx,
            event_tx: event_tx.clone(),
        };
        let join = std::thread::Builder::new()
            .name("stellatune-playback-actor".to_owned())
            .spawn(move || actor_loop(config, command_rx, event_tx))
            .map_err(|error| PlaybackControlError::failed("runtime", error.to_string()))?;
        Ok(Self {
            controller,
            join: Mutex::new(Some(join)),
        })
    }

    pub fn controller(&self) -> PlaybackController {
        self.controller.clone()
    }

    pub async fn shutdown(self) -> Result<(), PlaybackControlError> {
        self.controller.request(CommandKind::Shutdown).await?;
        if let Some(join) = self.join.lock().expect("runtime join poisoned").take() {
            join.join().map_err(|_| {
                PlaybackControlError::failed(
                    "runtime",
                    "playback actor panicked during shutdown".to_owned(),
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for PlaybackRuntime {
    fn drop(&mut self) {
        if self.join.lock().expect("runtime join poisoned").is_some() {
            let (response, _) = oneshot::channel();
            let _ = self.controller.command_tx.try_send(Command {
                kind: CommandKind::Shutdown,
                response,
            });
        }
    }
}

#[derive(Clone)]
pub struct PlaybackController {
    command_tx: Sender<Command>,
    event_tx: broadcast::Sender<PlaybackEvent>,
}

impl PlaybackController {
    async fn request(&self, kind: CommandKind) -> Result<CommandReply, PlaybackControlError> {
        let (response, receiver) = oneshot::channel();
        self.command_tx
            .try_send(Command { kind, response })
            .map_err(|error| match error {
                TrySendError::Disconnected(_) => PlaybackControlError::Closed,
                TrySendError::Full(_) => PlaybackControlError::failed(
                    "runtime",
                    "playback command queue is full".to_owned(),
                ),
            })?;
        receiver.await.map_err(|_| PlaybackControlError::Closed)?
    }

    pub async fn switch(
        &self,
        item: PlaybackItem,
        options: SwitchOptions,
    ) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Switch { item, options })
            .await
            .map(|_| ())
    }

    pub async fn queue_next(&self, item: PlaybackItem) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::QueueNext { item })
            .await
            .map(|_| ())
    }

    pub async fn play(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Play).await.map(|_| ())
    }

    pub async fn pause(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Pause).await.map(|_| ())
    }

    pub async fn seek(&self, position: MediaTime) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Seek(position)).await.map(|_| ())
    }

    pub async fn stop(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::Stop).await.map(|_| ())
    }

    pub async fn set_output_gain(
        &self,
        gain: f32,
        ramp: MediaTime,
    ) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::SetOutputGain {
            gain: gain.clamp(0.0, 1.0),
            ramp,
        })
        .await
        .map(|_| ())
    }

    pub async fn rebuild_output(&self) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::RebuildOutput).await.map(|_| ())
    }

    pub async fn set_policies(
        &self,
        policies: PlaybackPolicies,
    ) -> Result<(), PlaybackControlError> {
        self.request(CommandKind::SetPolicies(policies))
            .await
            .map(|_| ())
    }

    pub async fn snapshot(&self) -> Result<PlaybackRuntimeSnapshot, PlaybackControlError> {
        match self.request(CommandKind::Snapshot).await? {
            CommandReply::Snapshot(snapshot) => Ok(snapshot),
            CommandReply::Unit => Err(PlaybackControlError::failed(
                "runtime",
                "snapshot command returned no snapshot".to_owned(),
            )),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<PlaybackEvent> {
        self.event_tx.subscribe()
    }
}

struct Command {
    kind: CommandKind,
    response: oneshot::Sender<Result<CommandReply, PlaybackControlError>>,
}

enum CommandKind {
    Switch {
        item: PlaybackItem,
        options: SwitchOptions,
    },
    QueueNext {
        item: PlaybackItem,
    },
    Play,
    Pause,
    Seek(MediaTime),
    Stop,
    SetOutputGain {
        gain: f32,
        ramp: MediaTime,
    },
    SetPolicies(PlaybackPolicies),
    RebuildOutput,
    Snapshot,
    Shutdown,
}

enum CommandReply {
    Unit,
    Snapshot(PlaybackRuntimeSnapshot),
}

enum PreparationKind {
    Current {
        autoplay: bool,
        response: oneshot::Sender<Result<CommandReply, PlaybackControlError>>,
    },
    Next {
        response: oneshot::Sender<Result<CommandReply, PlaybackControlError>>,
    },
    Recovery {
        item_id: PlaybackItemId,
        checkpoint: MediaTime,
        resume_state: PlaybackState,
        attempt: usize,
    },
}

struct PreparationResult {
    generation: u64,
    kind: PreparationKind,
    result: Result<PreparedTrack, PlaybackControlError>,
}

const NORMALIZER_CHUNK_FRAMES: usize = 1024;

struct PcmNormalizer {
    source: AudioFormat,
    target: AudioFormat,
    resampler: Option<Async<f32>>,
    input_frames: u64,
    output_frames: u64,
    leading_frames_to_trim: usize,
    drained: bool,
}

impl PcmNormalizer {
    fn new(source: AudioFormat, target: AudioFormat) -> Result<Self, PlaybackControlError> {
        source
            .validate()
            .map_err(|message| PlaybackControlError::failed("normalizer", message.to_owned()))?;
        target
            .validate()
            .map_err(|message| PlaybackControlError::failed("normalizer", message.to_owned()))?;
        let resampler = if source.sample_rate == target.sample_rate {
            None
        } else {
            let params = SincInterpolationParameters {
                sinc_len: 128,
                f_cutoff: Some(0.94),
                oversampling_factor: 128,
                interpolation: SincInterpolationType::Linear,
                window: WindowFunction::Blackman,
            };
            Some(
                Async::<f32>::new_sinc(
                    target.sample_rate as f64 / source.sample_rate as f64,
                    2.0,
                    &params,
                    NORMALIZER_CHUNK_FRAMES,
                    usize::from(target.channels),
                    FixedAsync::Input,
                )
                .map_err(|error| PlaybackControlError::failed("normalizer", error.to_string()))?,
            )
        };
        let leading_frames_to_trim = resampler.as_ref().map_or(0, Resampler::output_delay);
        Ok(Self {
            source,
            target,
            resampler,
            input_frames: 0,
            output_frames: 0,
            leading_frames_to_trim,
            drained: false,
        })
    }

    fn process(&mut self, block: &mut AudioBlock) -> Result<(), PlaybackControlError> {
        if block.samples.is_empty() {
            block.format = self.target;
            return Ok(());
        }
        let source_channels = usize::from(self.source.channels);
        let target_channels = usize::from(self.target.channels);
        if block.format != self.source || !block.samples.len().is_multiple_of(source_channels) {
            return Err(PlaybackControlError::failed(
                "normalizer",
                "normalizer input format changed after planning".to_owned(),
            ));
        }
        let input = std::mem::take(&mut block.samples);
        self.input_frames = self
            .input_frames
            .saturating_add((input.len() / source_channels) as u64);
        let remapped = remap_channels(&input, source_channels, target_channels);
        block.samples = if let Some(resampler) = self.resampler.as_mut() {
            let mut output = Vec::new();
            let mut offset = 0;
            while offset < remapped.len() {
                let remaining_frames = (remapped.len() - offset) / target_channels;
                let frames = remaining_frames.min(NORMALIZER_CHUNK_FRAMES);
                if frames == 0 {
                    break;
                }
                let samples = frames.saturating_mul(target_channels);
                resampler.set_chunk_size(frames).map_err(|error| {
                    PlaybackControlError::failed("normalizer", error.to_string())
                })?;
                let adapter = InterleavedSlice::new(
                    &remapped[offset..offset + samples],
                    target_channels,
                    frames,
                )
                .map_err(|error| PlaybackControlError::failed("normalizer", error.to_string()))?;
                output.extend(
                    resampler
                        .process(&adapter, None)
                        .map_err(|error| {
                            PlaybackControlError::failed("normalizer", error.to_string())
                        })?
                        .take_data(),
                );
                offset += samples;
            }
            output
        } else {
            remapped
        };
        self.trim_leading_frames(&mut block.samples);
        self.output_frames = self
            .output_frames
            .saturating_add((block.samples.len() / target_channels) as u64);
        block.format = self.target;
        Ok(())
    }

    fn drain(&mut self, block: &mut AudioBlock) -> Result<bool, PlaybackControlError> {
        block.format = self.target;
        block.samples.clear();
        if self.drained || self.resampler.is_none() {
            self.drained = true;
            return Ok(false);
        }

        let expected_frames = ((self.input_frames as f64 * self.target.sample_rate as f64
            / self.source.sample_rate as f64)
            .ceil()) as u64;
        if self.output_frames >= expected_frames {
            self.drained = true;
            return Ok(false);
        }

        // A zero-length partial chunk asks rubato to advance its filter state using
        // silence. Repeat only until this turn yields audible data or the exact
        // resampled duration has been reached.
        while self.output_frames < expected_frames {
            let resampler = self.resampler.as_mut().expect("checked above");
            let channels = usize::from(self.target.channels);
            let input_frames = resampler.input_frames_next();
            let silence = vec![0.0; input_frames.saturating_mul(channels)];
            let adapter = InterleavedSlice::new(&silence, channels, input_frames)
                .map_err(|error| PlaybackControlError::failed("normalizer", error.to_string()))?;
            let mut output = resampler
                .process(&adapter, Some(&Indexing::new().partial_len(0)))
                .map_err(|error| PlaybackControlError::failed("normalizer", error.to_string()))?
                .take_data();
            self.trim_leading_frames(&mut output);
            let remaining_frames = expected_frames.saturating_sub(self.output_frames) as usize;
            output.truncate(remaining_frames.saturating_mul(channels));
            if !output.is_empty() {
                self.output_frames = self
                    .output_frames
                    .saturating_add((output.len() / channels) as u64);
                block.samples = output;
                return Ok(true);
            }
        }
        self.drained = true;
        Ok(false)
    }

    fn trim_leading_frames(&mut self, samples: &mut Vec<f32>) {
        if self.leading_frames_to_trim == 0 {
            return;
        }
        let channels = usize::from(self.target.channels);
        let available_frames = samples.len() / channels;
        let trim_frames = available_frames.min(self.leading_frames_to_trim);
        samples.drain(..trim_frames.saturating_mul(channels));
        self.leading_frames_to_trim -= trim_frames;
    }

    fn reset(&mut self) {
        if let Some(resampler) = self.resampler.as_mut() {
            resampler.reset();
            self.leading_frames_to_trim = resampler.output_delay();
        }
        self.input_frames = 0;
        self.output_frames = 0;
        self.drained = false;
    }
}

fn remap_channels(input: &[f32], source_channels: usize, target_channels: usize) -> Vec<f32> {
    if source_channels == target_channels {
        return input.to_vec();
    }
    let frames = input.len() / source_channels.max(1);
    let mut output = Vec::with_capacity(frames.saturating_mul(target_channels));
    for source in input.chunks_exact(source_channels.max(1)) {
        if target_channels == 1 {
            output.push(source.iter().copied().sum::<f32>() / source_channels.max(1) as f32);
            continue;
        }
        if source_channels == 1 {
            output.extend(std::iter::repeat_n(source[0], target_channels));
            continue;
        }
        for channel in 0..target_channels {
            output.push(source.get(channel).copied().unwrap_or(0.0));
        }
    }
    output
}

struct PreparedTrack {
    plan: ExecutablePlaybackPlan,
    decoder: Box<dyn DecoderStage>,
    pre_mix_transforms: Vec<Box<dyn TransformStage>>,
    pre_mix_formats: Vec<AudioFormat>,
    post_mix_transforms: Vec<Box<dyn TransformStage>>,
    post_mix_formats: Vec<AudioFormat>,
    decoded_format: AudioFormat,
    mix_format: AudioFormat,
    output_format: AudioFormat,
    normalizer: Option<PcmNormalizer>,
    duration_frames: Option<u64>,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    initial_decoded_frame: u64,
    initial_audible_frame: u64,
}

struct ActiveTrack {
    recovery_plan: ExecutablePlaybackPlan,
    item_id: PlaybackItemId,
    decoder: Box<dyn DecoderStage>,
    pre_mix_transforms: Vec<Box<dyn TransformStage>>,
    pre_mix_formats: Vec<AudioFormat>,
    post_mix_transforms: Vec<Box<dyn TransformStage>>,
    post_mix_formats: Vec<AudioFormat>,
    decoded_format: AudioFormat,
    mix_format: AudioFormat,
    output_format: AudioFormat,
    normalizer: Option<PcmNormalizer>,
    duration_frames: Option<u64>,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    tail_buffer: Vec<f32>,
    decoded_frame: u64,
    produced_audible_frame: u64,
    position_base_frame: u64,
    last_reported_position_frame: u64,
    epoch: u64,
    pending_block: Option<AudioBlock>,
    sink_factory: Arc<dyn SinkFactory>,
    output: SinkWorker,
    sink_consumed_base_frame: u64,
    boundary_announced: bool,
    transition: TransitionPolicy,
    fade_in_frames: u64,
    fade_in_start_frame: u64,
    recovery_fade: Option<TransitionRecoveryFade>,
    seek_fade_frames: u64,
    forced_end_frame: Option<u64>,
    drain_phase: DrainPhase,
}

#[derive(Debug, Clone, Copy)]
struct TransitionRecoveryFade {
    start_frame: u64,
    duration_frames: u64,
    start_gain: f32,
}

struct ActorState {
    state: PlaybackState,
    generation: u64,
    preparation_cancellation: SourceCancellation,
    current: Option<ActiveTrack>,
    next: Option<PreparedTrack>,
    next_preparing: bool,
    pending_current_response: Option<oneshot::Sender<Result<CommandReply, PlaybackControlError>>>,
    pending_next_response: Option<oneshot::Sender<Result<CommandReply, PlaybackControlError>>>,
    pending_seek: Option<PendingSeek>,
    crossfade: Option<CrossfadeState>,
    force_transition: bool,
    policies: PlaybackPolicies,
    output_gain: f32,
}

struct SecondaryTrack {
    recovery_plan: ExecutablePlaybackPlan,
    item_id: PlaybackItemId,
    decoder: Box<dyn DecoderStage>,
    pre_mix_transforms: Vec<Box<dyn TransformStage>>,
    pre_mix_formats: Vec<AudioFormat>,
    decoded_format: AudioFormat,
    mix_format: AudioFormat,
    normalizer: Option<PcmNormalizer>,
    duration_frames: Option<u64>,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    tail_buffer: Vec<f32>,
    decoded_frame: u64,
    produced_audible_frame: u64,
    sink_factory: Arc<dyn SinkFactory>,
    transition: TransitionPolicy,
    seek_fade_frames: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrainPhase {
    Decoding,
    PreMix(usize),
    Normalizer,
    PostMix(usize),
    Complete,
}

struct CrossfadeState {
    next: SecondaryTrack,
    duration_frames: u64,
    curve: CrossfadeCurve,
    progressed_frames: u64,
    current_block: Option<AudioBlock>,
    next_block: Option<AudioBlock>,
    sink_consumed_base_frame: u64,
    boundary_announced: bool,
}

struct PendingSeek {
    response: oneshot::Sender<Result<CommandReply, PlaybackControlError>>,
    resume_state: PlaybackState,
    item_id: PlaybackItemId,
}

fn actor_loop(
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

fn spawn_preparation(
    plan: ExecutablePlaybackPlan,
    generation: u64,
    purpose: SourceOpenPurpose,
    kind: PreparationKind,
    sender: Sender<PreparationResult>,
    cancellation: SourceCancellation,
) {
    std::thread::spawn(move || {
        let item_id = plan.item.id;
        let recovery = match &kind {
            PreparationKind::Recovery {
                checkpoint,
                attempt,
                ..
            } => Some((*checkpoint, *attempt)),
            _ => None,
        };
        if let Some((_, attempt)) = recovery {
            let backoff_ms = plan
                .policies
                .recovery_backoff_ms
                .saturating_mul(attempt.saturating_sub(1) as u64);
            if backoff_ms > 0 {
                let deadline = std::time::Instant::now() + Duration::from_millis(backoff_ms);
                while std::time::Instant::now() < deadline {
                    if cancellation.is_cancelled() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        let result = if cancellation.is_cancelled() {
            Err(PlaybackControlError::Closed)
        } else {
            prepare_track(
                plan,
                purpose,
                recovery.map(|(checkpoint, _)| checkpoint),
                cancellation,
            )
            .map_err(|error| error.with_context(Some(item_id), generation))
        };
        let _ = sender.send(PreparationResult {
            generation,
            kind,
            result,
        });
    });
}

fn prepare_track(
    plan: ExecutablePlaybackPlan,
    purpose: SourceOpenPurpose,
    resume_position: Option<MediaTime>,
    cancellation: SourceCancellation,
) -> Result<PreparedTrack, PlaybackControlError> {
    let capabilities = plan.item.source.descriptor().capabilities;
    let hints = plan.item.source.descriptor().media;
    let mut last_error = None;
    let fallback_limit = plan
        .policies
        .max_decoder_fallbacks
        .min(plan.decoder_candidates.len());

    for (index, factory) in plan
        .decoder_candidates
        .iter()
        .take(fallback_limit.max(1))
        .enumerate()
    {
        if !can_fallback(capabilities, index) {
            break;
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| PlaybackControlError::failed("runtime", error.to_string()))?;
        let source = match runtime.block_on(plan.item.source.open(SourceOpenRequest {
            purpose,
            deadline: None,
            cancellation: cancellation.clone(),
        })) {
            Ok(source) => source,
            Err(error) => {
                last_error = Some(("source", error.to_string()));
                continue;
            },
        };
        let mut decoder = match factory.create() {
            Ok(decoder) => decoder,
            Err(error) => {
                last_error = Some(("decoder", error.to_string()));
                continue;
            },
        };
        let info = match decoder.open(source, &hints) {
            Ok(info) => info,
            Err(error) => {
                last_error = Some(("decoder", error.to_string()));
                continue;
            },
        };
        let decoded_format = info.format;
        let mut mix_format = decoded_format;
        let mut pre_mix_transforms = Vec::new();
        let mut pre_mix_formats = Vec::new();
        let mut post_mix_factories = Vec::new();
        for transform_factory in &plan.transforms {
            match transform_factory.descriptor().placement {
                TransformPlacement::PreMix => {},
                TransformPlacement::PostMix => {
                    post_mix_factories.push(transform_factory);
                    continue;
                },
            }
            let mut transform = transform_factory
                .create()
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            mix_format = transform
                .configure(mix_format)
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            pre_mix_transforms.push(transform);
            pre_mix_formats.push(mix_format);
        }
        let preferred_mix_format = plan
            .sink
            .preferred_format(mix_format)
            .map_err(|error| PlaybackControlError::failed("sink", error.to_string()))?;
        preferred_mix_format
            .validate()
            .map_err(|message| PlaybackControlError::failed("sink", message.to_owned()))?;
        let normalizer = if preferred_mix_format == mix_format {
            None
        } else {
            let source = mix_format;
            mix_format = preferred_mix_format;
            Some(PcmNormalizer::new(source, mix_format)?)
        };
        let mut output_format = mix_format;
        let mut post_mix_transforms = Vec::with_capacity(post_mix_factories.len());
        let mut post_mix_formats = Vec::with_capacity(post_mix_factories.len());
        for transform_factory in post_mix_factories {
            let mut transform = transform_factory
                .create()
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            output_format = transform
                .configure(output_format)
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            post_mix_transforms.push(transform);
            post_mix_formats.push(output_format);
        }
        let trim_head_frames = info
            .gapless_trim
            .map(|trim| u64::from(trim.head_frames))
            .unwrap_or(0);
        let trim_tail_frames = info
            .gapless_trim
            .map(|trim| u64::from(trim.tail_frames))
            .unwrap_or(0);
        let duration_frames = info.duration_frames.map(|duration| {
            let decoded_frames =
                duration.saturating_sub(trim_head_frames.saturating_add(trim_tail_frames));
            MediaTime::from_frames(decoded_frames, decoded_format.sample_rate)
                .to_frames(mix_format.sample_rate)
        });
        let (initial_decoded_frame, initial_audible_frame) = if let Some(position) = resume_position
        {
            if !capabilities.byte_seekable {
                return Err(PlaybackControlError::Unsupported);
            }
            let target = position
                .to_frames(decoded_format.sample_rate)
                .saturating_add(trim_head_frames);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let actual = match decoder.start_seek(target) {
                Ok(DecoderSeekStatus::Complete(result)) => result.actual_frame,
                Ok(DecoderSeekStatus::Pending) => loop {
                    if std::time::Instant::now() >= deadline {
                        return Err(PlaybackControlError::failed(
                            "decoder",
                            "recovery seek timed out".to_owned(),
                        ));
                    }
                    match decoder.continue_seek() {
                        Ok(DecoderSeekStatus::Complete(result)) => break result.actual_frame,
                        Ok(DecoderSeekStatus::Pending) => std::thread::yield_now(),
                        Err(error) => {
                            return Err(PlaybackControlError::failed("decoder", error.to_string()));
                        },
                    }
                },
                Err(error) => {
                    return Err(PlaybackControlError::failed("decoder", error.to_string()));
                },
            };
            let audible_decoded = actual.saturating_sub(trim_head_frames);
            let audible_mix = MediaTime::from_frames(audible_decoded, decoded_format.sample_rate)
                .to_frames(mix_format.sample_rate);
            (actual, audible_mix)
        } else {
            (0, 0)
        };
        return Ok(PreparedTrack {
            plan,
            decoder,
            pre_mix_transforms,
            pre_mix_formats,
            post_mix_transforms,
            post_mix_formats,
            decoded_format,
            mix_format,
            output_format,
            normalizer,
            duration_frames,
            trim_head_frames,
            trim_tail_frames,
            raw_duration_frames: info.duration_frames,
            initial_decoded_frame,
            initial_audible_frame,
        });
    }
    let (stage, message) = last_error.unwrap_or(("decoder", "no decoder candidate".to_owned()));
    Err(PlaybackControlError::failed(stage, message))
}

fn advance_generation(actor: &mut ActorState) {
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

fn activate(
    prepared: PreparedTrack,
    config: &PlaybackRuntimeConfig,
    output_gain: f32,
) -> Result<ActiveTrack, PlaybackControlError> {
    let recovery_plan = prepared.plan.clone();
    let sink_factory = Arc::clone(&prepared.plan.sink);
    let output = SinkWorker::start(
        Arc::clone(&sink_factory),
        prepared.output_format,
        config.pcm_ring_blocks,
        output_gain,
    )?;
    Ok(ActiveTrack {
        recovery_plan,
        item_id: prepared.plan.item.id,
        decoder: prepared.decoder,
        pre_mix_transforms: prepared.pre_mix_transforms,
        pre_mix_formats: prepared.pre_mix_formats,
        post_mix_transforms: prepared.post_mix_transforms,
        post_mix_formats: prepared.post_mix_formats,
        decoded_format: prepared.decoded_format,
        mix_format: prepared.mix_format,
        output_format: prepared.output_format,
        normalizer: prepared.normalizer,
        duration_frames: prepared.duration_frames,
        trim_head_frames: prepared.trim_head_frames,
        trim_tail_frames: prepared.trim_tail_frames,
        raw_duration_frames: prepared.raw_duration_frames,
        tail_buffer: Vec::new(),
        decoded_frame: prepared.initial_decoded_frame,
        produced_audible_frame: prepared.initial_audible_frame,
        position_base_frame: prepared.initial_audible_frame,
        last_reported_position_frame: prepared.initial_audible_frame,
        epoch: 0,
        pending_block: None,
        sink_factory,
        output,
        sink_consumed_base_frame: 0,
        boundary_announced: true,
        transition: prepared.plan.policies.transition,
        fade_in_frames: 0,
        fade_in_start_frame: 0,
        recovery_fade: None,
        seek_fade_frames: prepared.plan.policies.seek_fade_frames,
        forced_end_frame: None,
        drain_phase: DrainPhase::Decoding,
    })
}

fn pump_once(
    config: &PlaybackRuntimeConfig,
    preparation_tx: &Sender<PreparationResult>,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut ActorState,
) {
    if let Some(message) = actor
        .current
        .as_ref()
        .and_then(|current| current.output.try_failure())
    {
        begin_recovery(config, preparation_tx, event_tx, actor, "sink", message);
        return;
    }
    maybe_start_crossfade(actor);
    if actor.crossfade.is_some() {
        pump_crossfade(config, preparation_tx, event_tx, actor);
        return;
    }
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    while let Some(item_id) = current.output.try_boundary() {
        if item_id == current.item_id && !current.boundary_announced {
            current.boundary_announced = true;
            let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
        }
    }
    emit_position_if_due(current, event_tx, false);
    if current
        .forced_end_frame
        .is_some_and(|end| current.produced_audible_frame >= end)
    {
        promote_or_end(actor, config, event_tx);
        return;
    }
    if let Some(block) = current.pending_block.take() {
        match current.output.try_write(block) {
            Ok(()) => {},
            Err(PendingWrite::Full(block)) => {
                current.pending_block = Some(block);
                return;
            },
            Err(PendingWrite::Closed) => {
                begin_recovery(
                    config,
                    preparation_tx,
                    event_tx,
                    actor,
                    "sink",
                    "sink worker closed".to_owned(),
                );
                return;
            },
        }
    }

    let mut block = AudioBlock::new(current.decoded_format);
    block.timeline.start_frame = current.decoded_frame;
    block.timeline.epoch = current.epoch;
    block.samples.reserve(
        config
            .block_frames
            .saturating_mul(usize::from(current.decoded_format.channels)),
    );
    match current.decoder.decode(&mut block) {
        Ok(DecodeStatus::Produced { frames }) => {
            if frames == 0 || block.samples.is_empty() {
                return;
            }
            let raw_frames = block.frames() as u64;
            let raw_start = current.decoded_frame;
            current.decoded_frame = current.decoded_frame.saturating_add(raw_frames);
            trim_gapless_block(current, &mut block, raw_start);
            if block.samples.is_empty() {
                return;
            }
            block.timeline.start_frame = current.produced_audible_frame;
            if let Err(error) = process_transform_chain(
                &mut current.pre_mix_transforms,
                &current.pre_mix_formats,
                &mut block,
            ) {
                fail_current(actor, event_tx, "transform", error.to_string());
                return;
            }
            if block.samples.is_empty() {
                return;
            }
            if let Some(normalizer) = current.normalizer.as_mut()
                && let Err(error) = normalizer.process(&mut block)
            {
                fail_current(actor, event_tx, "normalizer", error.to_string());
                return;
            }
            apply_track_transition_gain(current, &mut block);
            current.produced_audible_frame = current
                .produced_audible_frame
                .saturating_add(block.frames() as u64);
            if let Err(error) = process_transform_chain(
                &mut current.post_mix_transforms,
                &current.post_mix_formats,
                &mut block,
            ) {
                fail_current(actor, event_tx, "transform", error.to_string());
                return;
            }
            if block.samples.is_empty() {
                return;
            }
            let item_id = current.item_id;
            match current.output.try_write(block) {
                Ok(()) => {},
                Err(PendingWrite::Full(block)) => current.pending_block = Some(block),
                Err(PendingWrite::Closed) => {
                    begin_recovery(
                        config,
                        preparation_tx,
                        event_tx,
                        actor,
                        "sink",
                        "sink worker closed".to_owned(),
                    );
                },
            }
            if actor.state == PlaybackState::Buffering {
                set_state(actor, PlaybackState::Playing, event_tx);
                let _ = event_tx.send(PlaybackEvent::Buffering {
                    item_id,
                    active: false,
                });
            }
        },
        Ok(DecodeStatus::Pending) => {
            let item_id = current.item_id;
            set_state(actor, PlaybackState::Buffering, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id,
                active: true,
            });
        },
        Ok(DecodeStatus::EndOfStream) => match drain_current_once(current) {
            Ok(DrainTurn::Produced(block)) => match current.output.try_write(block) {
                Ok(()) => {},
                Err(PendingWrite::Full(block)) => current.pending_block = Some(block),
                Err(PendingWrite::Closed) => begin_recovery(
                    config,
                    preparation_tx,
                    event_tx,
                    actor,
                    "sink",
                    "sink worker closed".to_owned(),
                ),
            },
            Ok(DrainTurn::Pending) => {},
            Ok(DrainTurn::Complete) => promote_or_end(actor, config, event_tx),
            Err(error) => fail_current(actor, event_tx, "transform", error.to_string()),
        },
        Err(stellatune_audio_core::DecodeError::Io(error)) => begin_recovery(
            config,
            preparation_tx,
            event_tx,
            actor,
            "decoder",
            error.to_string(),
        ),
        Err(error) => fail_current(actor, event_tx, "decoder", error.to_string()),
    }
}

fn begin_recovery(
    _config: &PlaybackRuntimeConfig,
    preparation_tx: &Sender<PreparationResult>,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut ActorState,
    stage: &'static str,
    message: String,
) {
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    let capabilities = current.recovery_plan.item.source.descriptor().capabilities;
    if !capabilities.reopenable
        || !capabilities.byte_seekable
        || actor.policies.max_recovery_attempts == 0
    {
        fail_current(actor, event_tx, stage, message);
        return;
    }
    let clock = current.output.clock();
    let checkpoint_frame = current.position_base_frame.saturating_add(
        clock
            .consumed_frames
            .saturating_sub(current.sink_consumed_base_frame),
    );
    let checkpoint = MediaTime::from_frames(checkpoint_frame, current.mix_format.sample_rate);
    let item_id = current.item_id;
    let plan = current.recovery_plan.clone();
    let _ = current.output.pause();

    advance_generation(actor);
    actor.next = None;
    actor.next_preparing = false;
    actor.crossfade = None;
    actor.force_transition = false;
    if let Some(pending) = actor.pending_seek.take() {
        let _ = pending.response.send(Err(PlaybackControlError::Closed));
    }
    set_state(actor, PlaybackState::Recovering, event_tx);
    spawn_preparation(
        plan,
        actor.generation,
        SourceOpenPurpose::Recovery,
        PreparationKind::Recovery {
            item_id,
            checkpoint,
            resume_state: PlaybackState::Playing,
            attempt: 1,
        },
        preparation_tx.clone(),
        actor.preparation_cancellation.clone(),
    );
}

fn promote_or_end(
    actor: &mut ActorState,
    config: &PlaybackRuntimeConfig,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    actor.force_transition = false;
    let Some(mut ended) = actor.current.take() else {
        return;
    };
    let ended_item_id = ended.item_id;
    let Some(mut next) = actor.next.take() else {
        if actor.next_preparing {
            actor.current = Some(ended);
            set_state(actor, PlaybackState::Buffering, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id: ended_item_id,
                active: true,
            });
            return;
        }
        let _ = ended.output.drain();
        ended.output.shutdown();
        set_state(actor, PlaybackState::Idle, event_tx);
        let _ = event_tx.send(PlaybackEvent::PlaybackEnded {
            item_id: ended_item_id,
        });
        return;
    };

    let normalized = normalize_prepared_for_mix(&mut next, ended.mix_format).is_ok();
    let current_key = ended
        .sink_factory
        .compatibility_key(ended.output_format)
        .ok();
    let next_key = next.plan.sink.compatibility_key(next.output_format).ok();
    let compatible = normalized
        && ended.mix_format == next.mix_format
        && ended.output_format == next.output_format
        && current_key.is_some()
        && current_key == next_key;
    if compatible {
        let clock = ended.output.clock();
        let next_base = clock.consumed_frames.saturating_add(clock.buffered_frames);
        let next_item_id = next.plan.item.id;
        if ended.output.mark_boundary(next_item_id).is_err() {
            ended.output.shutdown();
            fail_promoted(actor, event_tx, "failed to queue item boundary".to_owned());
            return;
        }
        ended.decoder.reset();
        for transform in &mut ended.pre_mix_transforms {
            transform.reset();
        }
        if let Some(normalizer) = ended.normalizer.as_mut() {
            normalizer.reset();
        }
        let fade_in_frames = transition_fade_in_frames(ended.transition);
        let output = ended.output;
        let promoted = activate_with_output(
            next,
            output,
            ended.post_mix_transforms,
            ended.post_mix_formats,
            next_base,
            false,
            fade_in_frames,
        );
        actor.current = Some(promoted);
        set_state(actor, PlaybackState::Playing, event_tx);
    } else {
        let _ = ended.output.drain();
        ended.output.shutdown();
        match activate(next, config, actor.output_gain) {
            Ok(promoted) => {
                let item_id = promoted.item_id;
                let mut promoted = promoted;
                promoted.fade_in_frames = transition_fade_in_frames(ended.transition);
                let _ = promoted.output.resume();
                actor.current = Some(promoted);
                set_state(actor, PlaybackState::Playing, event_tx);
                let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
            },
            Err(error) => fail_promoted(actor, event_tx, error.to_string()),
        }
    }
}

enum TrackBlockStatus {
    Data(AudioBlock),
    Pending,
    EndOfStream,
}

fn maybe_start_crossfade(actor: &mut ActorState) {
    if actor.crossfade.is_some() {
        return;
    }
    let forced = actor.force_transition;
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    let TransitionPolicy::Crossfade {
        duration_frames,
        curve,
        ..
    } = current.transition
    else {
        return;
    };
    let Some(duration) = current.duration_frames else {
        return;
    };
    if duration_frames == 0 || current.pending_block.is_some() {
        return;
    }
    let clock = current.output.clock();
    let produced_frontier = current
        .position_base_frame
        .saturating_add(
            clock
                .consumed_frames
                .saturating_sub(current.sink_consumed_base_frame),
        )
        .saturating_add(clock.buffered_frames);
    if !forced && produced_frontier < duration.saturating_sub(duration_frames) {
        return;
    }
    let Some(next) = actor.next.as_mut() else {
        return;
    };
    if normalize_prepared_for_mix(next, current.mix_format).is_err() {
        return;
    }
    let compatible = current.mix_format == next.mix_format
        && current.output_format == next.output_format
        && current
            .sink_factory
            .compatibility_key(current.output_format)
            .ok()
            == next.plan.sink.compatibility_key(next.output_format).ok();
    if !compatible {
        return;
    }
    let next = actor.next.take().expect("next checked above");
    let item_id = next.plan.item.id;
    let boundary_base = clock.consumed_frames.saturating_add(clock.buffered_frames);
    if current.output.mark_boundary(item_id).is_err() {
        actor.next = Some(next);
        return;
    }
    actor.crossfade = Some(CrossfadeState {
        next: secondary_from_prepared(next),
        duration_frames,
        curve,
        progressed_frames: 0,
        current_block: None,
        next_block: None,
        sink_consumed_base_frame: boundary_base,
        boundary_announced: false,
    });
    actor.force_transition = false;
}

fn normalize_prepared_for_mix(
    prepared: &mut PreparedTrack,
    target: AudioFormat,
) -> Result<(), PlaybackControlError> {
    if prepared.mix_format == target {
        return Ok(());
    }
    let source = prepared.mix_format;
    let normalizer = PcmNormalizer::new(source, target)?;
    let mut post_mix_transforms = Vec::new();
    let mut post_mix_formats = Vec::new();
    let mut output_format = target;
    for factory in &prepared.plan.transforms {
        if factory.descriptor().placement != TransformPlacement::PostMix {
            continue;
        }
        let mut transform = factory
            .create()
            .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
        output_format = transform
            .configure(output_format)
            .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
        post_mix_transforms.push(transform);
        post_mix_formats.push(output_format);
    }
    prepared.duration_frames = prepared.duration_frames.map(|frames| {
        MediaTime::from_frames(frames, source.sample_rate).to_frames(target.sample_rate)
    });
    prepared.normalizer = Some(normalizer);
    prepared.mix_format = target;
    prepared.output_format = output_format;
    prepared.post_mix_transforms = post_mix_transforms;
    prepared.post_mix_formats = post_mix_formats;
    Ok(())
}

fn secondary_from_prepared(prepared: PreparedTrack) -> SecondaryTrack {
    let recovery_plan = prepared.plan.clone();
    SecondaryTrack {
        recovery_plan,
        item_id: prepared.plan.item.id,
        decoder: prepared.decoder,
        pre_mix_transforms: prepared.pre_mix_transforms,
        pre_mix_formats: prepared.pre_mix_formats,
        decoded_format: prepared.decoded_format,
        mix_format: prepared.mix_format,
        normalizer: prepared.normalizer,
        duration_frames: prepared.duration_frames,
        trim_head_frames: prepared.trim_head_frames,
        trim_tail_frames: prepared.trim_tail_frames,
        raw_duration_frames: prepared.raw_duration_frames,
        tail_buffer: Vec::new(),
        decoded_frame: 0,
        produced_audible_frame: 0,
        sink_factory: prepared.plan.sink,
        transition: prepared.plan.policies.transition,
        seek_fade_frames: prepared.plan.policies.seek_fade_frames,
    }
}

fn pump_crossfade(
    config: &PlaybackRuntimeConfig,
    preparation_tx: &Sender<PreparationResult>,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    actor: &mut ActorState,
) {
    let Some(current) = actor.current.as_mut() else {
        actor.crossfade = None;
        return;
    };
    let Some(crossfade) = actor.crossfade.as_mut() else {
        return;
    };

    while let Some(item_id) = current.output.try_boundary() {
        if item_id == crossfade.next.item_id && !crossfade.boundary_announced {
            crossfade.boundary_announced = true;
            let _ = event_tx.send(PlaybackEvent::TrackChanged { item_id });
        }
    }
    if let Some(block) = current.pending_block.take() {
        match current.output.try_write(block) {
            Ok(()) => {},
            Err(PendingWrite::Full(block)) => {
                current.pending_block = Some(block);
                return;
            },
            Err(PendingWrite::Closed) => {
                begin_recovery(
                    config,
                    preparation_tx,
                    event_tx,
                    actor,
                    "sink",
                    "sink worker closed".to_owned(),
                );
                return;
            },
        }
    }

    if crossfade.current_block.is_none() {
        match decode_track_block(
            current.decoder.as_mut(),
            &mut current.pre_mix_transforms,
            &current.pre_mix_formats,
            &mut current.normalizer,
            current.decoded_format,
            current.trim_head_frames,
            current.trim_tail_frames,
            current.raw_duration_frames,
            &mut current.tail_buffer,
            &mut current.decoded_frame,
            &mut current.produced_audible_frame,
            config.block_frames,
            current.epoch,
        ) {
            Ok(TrackBlockStatus::Data(block)) => crossfade.current_block = Some(block),
            Ok(TrackBlockStatus::Pending) => {
                set_state(actor, PlaybackState::Buffering, event_tx);
                return;
            },
            Ok(TrackBlockStatus::EndOfStream) => {
                crossfade.progressed_frames = crossfade.duration_frames;
            },
            Err(PlaybackControlError::Failed(failure))
                if failure.stage == stellatune_audio_core::FailureStage::Decoder =>
            {
                begin_recovery(
                    config,
                    preparation_tx,
                    event_tx,
                    actor,
                    "decoder",
                    failure.message,
                );
                return;
            },
            Err(error) => {
                fail_current(actor, event_tx, "decoder", error.to_string());
                return;
            },
        }
    }
    if crossfade.progressed_frames >= crossfade.duration_frames {
        finish_crossfade(actor, event_tx);
        return;
    }
    if crossfade.next_block.is_none() {
        let next_failure =
            match decode_secondary_block(&mut crossfade.next, config.block_frames, current.epoch) {
                Ok(TrackBlockStatus::Data(block)) => {
                    crossfade.next_block = Some(block);
                    None
                },
                Ok(TrackBlockStatus::Pending) => {
                    set_state(actor, PlaybackState::Buffering, event_tx);
                    return;
                },
                Ok(TrackBlockStatus::EndOfStream) => Some(PlaybackFailure::internal(
                    "decoder",
                    "next track ended during an active crossfade",
                )),
                Err(PlaybackControlError::Failed(failure)) => Some(failure),
                Err(error) => Some(PlaybackFailure::internal("decoder", error.to_string())),
            };
        if let Some(failure) = next_failure {
            let progress =
                crossfade.progressed_frames as f32 / crossfade.duration_frames.max(1) as f32;
            let (current_gain, _) = crossfade_gains(progress.clamp(0.0, 1.0), crossfade.curve);
            let remaining = crossfade
                .duration_frames
                .saturating_sub(crossfade.progressed_frames)
                .max(1);
            if let Some(mut block) = crossfade.current_block.take() {
                current.recovery_fade = Some(TransitionRecoveryFade {
                    start_frame: block.timeline.start_frame,
                    duration_frames: remaining,
                    start_gain: current_gain,
                });
                apply_track_transition_gain(current, &mut block);
                if let Err(error) = process_transform_chain(
                    &mut current.post_mix_transforms,
                    &current.post_mix_formats,
                    &mut block,
                ) {
                    fail_current(actor, event_tx, "transform", error.to_string());
                    return;
                }
                if !block.samples.is_empty() {
                    current.pending_block = Some(block);
                }
            }
            let failure = failure.with_context(Some(crossfade.next.item_id), actor.generation);
            let _ = event_tx.send(PlaybackEvent::Failed(failure));
            actor.crossfade = None;
            set_state(actor, PlaybackState::Playing, event_tx);
            return;
        }
    }
    let current_frames = crossfade
        .current_block
        .as_ref()
        .map(AudioBlock::frames)
        .unwrap_or(0);
    let next_frames = crossfade
        .next_block
        .as_ref()
        .map(AudioBlock::frames)
        .unwrap_or(0);
    let frames = current_frames.min(next_frames).min(
        crossfade
            .duration_frames
            .saturating_sub(crossfade.progressed_frames) as usize,
    );
    if frames == 0 {
        return;
    }
    let channels = usize::from(current.mix_format.channels.max(1));
    let sample_count = frames.saturating_mul(channels);
    let mut mixed = AudioBlock::new(current.mix_format);
    mixed.timeline.start_frame = current
        .produced_audible_frame
        .saturating_sub(current_frames as u64);
    mixed.timeline.epoch = current.epoch;
    mixed.samples.reserve(sample_count);
    let current_samples = &crossfade.current_block.as_ref().unwrap().samples[..sample_count];
    let next_samples = &crossfade.next_block.as_ref().unwrap().samples[..sample_count];
    for frame in 0..frames {
        let progress = (crossfade.progressed_frames.saturating_add(frame as u64) as f32
            / crossfade.duration_frames.max(1) as f32)
            .clamp(0.0, 1.0);
        let (gain_a, gain_b) = crossfade_gains(progress, crossfade.curve);
        let offset = frame.saturating_mul(channels);
        for channel in 0..channels {
            mixed.samples.push(
                current_samples[offset + channel] * gain_a
                    + next_samples[offset + channel] * gain_b,
            );
        }
    }
    consume_block_prefix(&mut crossfade.current_block, sample_count, frames as u64);
    consume_block_prefix(&mut crossfade.next_block, sample_count, frames as u64);
    crossfade.progressed_frames = crossfade.progressed_frames.saturating_add(frames as u64);
    if let Err(error) = process_transform_chain(
        &mut current.post_mix_transforms,
        &current.post_mix_formats,
        &mut mixed,
    ) {
        fail_current(actor, event_tx, "transform", error.to_string());
        return;
    }
    if mixed.samples.is_empty() {
        return;
    }
    if actor.state == PlaybackState::Buffering {
        actor.state = PlaybackState::Playing;
        let _ = event_tx.send(PlaybackEvent::StateChanged(PlaybackState::Playing));
    }
    match current.output.try_write(mixed) {
        Ok(()) => {},
        Err(PendingWrite::Full(block)) => current.pending_block = Some(block),
        Err(PendingWrite::Closed) => {
            begin_recovery(
                config,
                preparation_tx,
                event_tx,
                actor,
                "sink",
                "sink worker closed".to_owned(),
            );
            return;
        },
    }
    if crossfade.progressed_frames >= crossfade.duration_frames {
        finish_crossfade(actor, event_tx);
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_track_block(
    decoder: &mut dyn DecoderStage,
    transforms: &mut [Box<dyn TransformStage>],
    transform_formats: &[AudioFormat],
    normalizer: &mut Option<PcmNormalizer>,
    format: AudioFormat,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    tail_buffer: &mut Vec<f32>,
    decoded_frame: &mut u64,
    produced_audible_frame: &mut u64,
    block_frames: usize,
    epoch: u64,
) -> Result<TrackBlockStatus, PlaybackControlError> {
    let mut block = AudioBlock::new(format);
    block.timeline.start_frame = *decoded_frame;
    block.timeline.epoch = epoch;
    block
        .samples
        .reserve(block_frames.saturating_mul(usize::from(format.channels)));
    match decoder
        .decode(&mut block)
        .map_err(|error| PlaybackControlError::failed("decoder", error.to_string()))?
    {
        DecodeStatus::Produced { frames } if frames > 0 && !block.samples.is_empty() => {
            let raw_start = *decoded_frame;
            *decoded_frame = decoded_frame.saturating_add(block.frames() as u64);
            trim_gapless_samples(
                &mut block,
                raw_start,
                trim_head_frames,
                trim_tail_frames,
                raw_duration_frames,
                tail_buffer,
            );
            if block.samples.is_empty() {
                return Ok(TrackBlockStatus::Pending);
            }
            block.timeline.start_frame = *produced_audible_frame;
            process_transform_chain(transforms, transform_formats, &mut block)
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
            if block.samples.is_empty() {
                return Ok(TrackBlockStatus::Pending);
            }
            if let Some(normalizer) = normalizer.as_mut() {
                normalizer.process(&mut block)?;
            }
            *produced_audible_frame = produced_audible_frame.saturating_add(block.frames() as u64);
            Ok(TrackBlockStatus::Data(block))
        },
        DecodeStatus::Produced { .. } | DecodeStatus::Pending => Ok(TrackBlockStatus::Pending),
        DecodeStatus::EndOfStream => Ok(TrackBlockStatus::EndOfStream),
    }
}

fn process_transform_chain(
    transforms: &mut [Box<dyn TransformStage>],
    formats: &[AudioFormat],
    block: &mut AudioBlock,
) -> Result<(), stellatune_audio_core::TransformError> {
    debug_assert_eq!(transforms.len(), formats.len());
    for (transform, output_format) in transforms.iter_mut().zip(formats) {
        match transform.process(block)? {
            TransformStatus::Produced => block.format = *output_format,
            TransformStatus::Buffered => {
                block.samples.clear();
                return Ok(());
            },
        }
    }
    Ok(())
}

enum DrainTurn {
    Produced(AudioBlock),
    Pending,
    Complete,
}

fn drain_current_once(current: &mut ActiveTrack) -> Result<DrainTurn, PlaybackControlError> {
    loop {
        match current.drain_phase {
            DrainPhase::Decoding => current.drain_phase = DrainPhase::PreMix(0),
            DrainPhase::PreMix(index) if index >= current.pre_mix_transforms.len() => {
                current.drain_phase = DrainPhase::Normalizer;
            },
            DrainPhase::PreMix(index) => {
                let mut block = AudioBlock::new(current.pre_mix_formats[index]);
                block.timeline.start_frame = current.produced_audible_frame;
                block.timeline.epoch = current.epoch;
                match current.pre_mix_transforms[index]
                    .drain(&mut block)
                    .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?
                {
                    stellatune_audio_core::DrainStatus::Complete => {
                        current.drain_phase = DrainPhase::PreMix(index + 1);
                    },
                    stellatune_audio_core::DrainStatus::Produced => {
                        block.format = current.pre_mix_formats[index];
                        process_transform_chain(
                            &mut current.pre_mix_transforms[index + 1..],
                            &current.pre_mix_formats[index + 1..],
                            &mut block,
                        )
                        .map_err(|error| {
                            PlaybackControlError::failed("transform", error.to_string())
                        })?;
                        if block.samples.is_empty() {
                            return Ok(DrainTurn::Pending);
                        }
                        if let Some(normalizer) = current.normalizer.as_mut() {
                            normalizer.process(&mut block)?;
                        }
                        apply_track_transition_gain(current, &mut block);
                        current.produced_audible_frame = current
                            .produced_audible_frame
                            .saturating_add(block.frames() as u64);
                        process_transform_chain(
                            &mut current.post_mix_transforms,
                            &current.post_mix_formats,
                            &mut block,
                        )
                        .map_err(|error| {
                            PlaybackControlError::failed("transform", error.to_string())
                        })?;
                        return Ok(if block.samples.is_empty() {
                            DrainTurn::Pending
                        } else {
                            DrainTurn::Produced(block)
                        });
                    },
                }
            },
            DrainPhase::Normalizer => {
                let Some(normalizer) = current.normalizer.as_mut() else {
                    current.drain_phase = DrainPhase::PostMix(0);
                    continue;
                };
                let mut block = AudioBlock::new(current.mix_format);
                block.timeline.start_frame = current.produced_audible_frame;
                block.timeline.epoch = current.epoch;
                if !normalizer.drain(&mut block)? {
                    current.drain_phase = DrainPhase::PostMix(0);
                    continue;
                }
                apply_track_transition_gain(current, &mut block);
                current.produced_audible_frame = current
                    .produced_audible_frame
                    .saturating_add(block.frames() as u64);
                process_transform_chain(
                    &mut current.post_mix_transforms,
                    &current.post_mix_formats,
                    &mut block,
                )
                .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?;
                return Ok(if block.samples.is_empty() {
                    DrainTurn::Pending
                } else {
                    DrainTurn::Produced(block)
                });
            },
            DrainPhase::PostMix(index) if index >= current.post_mix_transforms.len() => {
                current.drain_phase = DrainPhase::Complete;
            },
            DrainPhase::PostMix(index) => {
                let mut block = AudioBlock::new(current.post_mix_formats[index]);
                block.timeline.start_frame = current.produced_audible_frame;
                block.timeline.epoch = current.epoch;
                match current.post_mix_transforms[index]
                    .drain(&mut block)
                    .map_err(|error| PlaybackControlError::failed("transform", error.to_string()))?
                {
                    stellatune_audio_core::DrainStatus::Complete => {
                        current.drain_phase = DrainPhase::PostMix(index + 1);
                    },
                    stellatune_audio_core::DrainStatus::Produced => {
                        block.format = current.post_mix_formats[index];
                        process_transform_chain(
                            &mut current.post_mix_transforms[index + 1..],
                            &current.post_mix_formats[index + 1..],
                            &mut block,
                        )
                        .map_err(|error| {
                            PlaybackControlError::failed("transform", error.to_string())
                        })?;
                        return Ok(if block.samples.is_empty() {
                            DrainTurn::Pending
                        } else {
                            DrainTurn::Produced(block)
                        });
                    },
                }
            },
            DrainPhase::Complete => return Ok(DrainTurn::Complete),
        }
    }
}

fn decode_secondary_block(
    next: &mut SecondaryTrack,
    block_frames: usize,
    epoch: u64,
) -> Result<TrackBlockStatus, PlaybackControlError> {
    decode_track_block(
        next.decoder.as_mut(),
        &mut next.pre_mix_transforms,
        &next.pre_mix_formats,
        &mut next.normalizer,
        next.decoded_format,
        next.trim_head_frames,
        next.trim_tail_frames,
        next.raw_duration_frames,
        &mut next.tail_buffer,
        &mut next.decoded_frame,
        &mut next.produced_audible_frame,
        block_frames,
        epoch,
    )
}

fn consume_block_prefix(block: &mut Option<AudioBlock>, samples: usize, frames: u64) {
    let Some(value) = block.as_mut() else {
        return;
    };
    value.samples.drain(..samples);
    value.timeline.start_frame = value.timeline.start_frame.saturating_add(frames);
    if value.samples.is_empty() {
        *block = None;
    }
}

fn crossfade_gains(progress: f32, curve: CrossfadeCurve) -> (f32, f32) {
    match curve {
        CrossfadeCurve::Linear => (1.0 - progress, progress),
        CrossfadeCurve::EqualPower => {
            let phase = progress * std::f32::consts::FRAC_PI_2;
            (phase.cos(), phase.sin())
        },
    }
}

fn finish_crossfade(actor: &mut ActorState, event_tx: &broadcast::Sender<PlaybackEvent>) {
    let Some(crossfade) = actor.crossfade.take() else {
        return;
    };
    let Some(mut ended) = actor.current.take() else {
        return;
    };
    ended.decoder.reset();
    for transform in &mut ended.pre_mix_transforms {
        transform.reset();
    }
    if let Some(normalizer) = ended.normalizer.as_mut() {
        normalizer.reset();
    }
    let output = ended.output;
    let next = crossfade.next;
    actor.current = Some(ActiveTrack {
        recovery_plan: next.recovery_plan,
        item_id: next.item_id,
        decoder: next.decoder,
        pre_mix_transforms: next.pre_mix_transforms,
        pre_mix_formats: next.pre_mix_formats,
        post_mix_transforms: ended.post_mix_transforms,
        post_mix_formats: ended.post_mix_formats,
        decoded_format: next.decoded_format,
        mix_format: next.mix_format,
        output_format: ended.output_format,
        normalizer: next.normalizer,
        duration_frames: next.duration_frames,
        trim_head_frames: next.trim_head_frames,
        trim_tail_frames: next.trim_tail_frames,
        raw_duration_frames: next.raw_duration_frames,
        tail_buffer: next.tail_buffer,
        decoded_frame: next.decoded_frame,
        produced_audible_frame: next.produced_audible_frame,
        position_base_frame: 0,
        last_reported_position_frame: 0,
        epoch: ended.epoch,
        pending_block: ended.pending_block,
        sink_factory: next.sink_factory,
        output,
        sink_consumed_base_frame: crossfade.sink_consumed_base_frame,
        boundary_announced: crossfade.boundary_announced,
        transition: next.transition,
        fade_in_frames: 0,
        fade_in_start_frame: 0,
        recovery_fade: None,
        seek_fade_frames: next.seek_fade_frames,
        forced_end_frame: None,
        drain_phase: DrainPhase::Decoding,
    });
    set_state(actor, PlaybackState::Playing, event_tx);
}

fn configure_forced_transition(actor: &mut ActorState) {
    if !actor.force_transition {
        return;
    }
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    match current.transition {
        TransitionPolicy::Gapless => {
            current.duration_frames = Some(current.produced_audible_frame);
            current.forced_end_frame = Some(current.produced_audible_frame);
        },
        TransitionPolicy::FadeOutIn {
            fade_out_frames, ..
        } => {
            let end = current
                .produced_audible_frame
                .saturating_add(fade_out_frames);
            current.duration_frames = Some(end);
            current.forced_end_frame = Some(end);
        },
        TransitionPolicy::Crossfade { .. } => {},
    }
}

fn activate_with_output(
    prepared: PreparedTrack,
    output: SinkWorker,
    post_mix_transforms: Vec<Box<dyn TransformStage>>,
    post_mix_formats: Vec<AudioFormat>,
    sink_consumed_base_frame: u64,
    boundary_announced: bool,
    fade_in_frames: u64,
) -> ActiveTrack {
    let recovery_plan = prepared.plan.clone();
    let transition = prepared.plan.policies.transition;
    let seek_fade_frames = prepared.plan.policies.seek_fade_frames;
    ActiveTrack {
        recovery_plan,
        item_id: prepared.plan.item.id,
        decoder: prepared.decoder,
        pre_mix_transforms: prepared.pre_mix_transforms,
        pre_mix_formats: prepared.pre_mix_formats,
        post_mix_transforms,
        post_mix_formats,
        decoded_format: prepared.decoded_format,
        mix_format: prepared.mix_format,
        output_format: prepared.output_format,
        normalizer: prepared.normalizer,
        duration_frames: prepared.duration_frames,
        trim_head_frames: prepared.trim_head_frames,
        trim_tail_frames: prepared.trim_tail_frames,
        raw_duration_frames: prepared.raw_duration_frames,
        tail_buffer: Vec::new(),
        decoded_frame: prepared.initial_decoded_frame,
        produced_audible_frame: prepared.initial_audible_frame,
        position_base_frame: prepared.initial_audible_frame,
        last_reported_position_frame: prepared.initial_audible_frame,
        epoch: 0,
        pending_block: None,
        sink_factory: prepared.plan.sink,
        output,
        sink_consumed_base_frame,
        boundary_announced,
        transition,
        fade_in_frames,
        fade_in_start_frame: 0,
        recovery_fade: None,
        seek_fade_frames,
        forced_end_frame: None,
        drain_phase: DrainPhase::Decoding,
    }
}

fn transition_fade_in_frames(transition: TransitionPolicy) -> u64 {
    match transition {
        TransitionPolicy::FadeOutIn { fade_in_frames, .. } => fade_in_frames,
        TransitionPolicy::Crossfade {
            fallback: crate::planner::CrossfadeFallback::FadeOutIn,
            duration_frames,
            ..
        } => duration_frames,
        _ => 0,
    }
}

fn apply_track_transition_gain(current: &ActiveTrack, block: &mut AudioBlock) {
    let channels = usize::from(block.format.channels.max(1));
    let fade_out = match current.transition {
        TransitionPolicy::FadeOutIn {
            fade_out_frames,
            curve,
            ..
        } => Some((fade_out_frames, curve)),
        TransitionPolicy::Crossfade {
            duration_frames,
            fallback: crate::planner::CrossfadeFallback::FadeOutIn,
            ..
        } => Some((duration_frames, GainCurve::Linear)),
        _ => None,
    };
    for (frame_index, frame) in block.samples.chunks_exact_mut(channels).enumerate() {
        let timeline_frame = block
            .timeline
            .start_frame
            .saturating_add(frame_index as u64);
        let mut gain = 1.0_f32;
        if current.fade_in_frames > 0
            && timeline_frame >= current.fade_in_start_frame
            && timeline_frame
                < current
                    .fade_in_start_frame
                    .saturating_add(current.fade_in_frames)
        {
            gain *= curve_gain(
                timeline_frame.saturating_sub(current.fade_in_start_frame) as f32
                    / current.fade_in_frames.max(1) as f32,
                GainCurve::Linear,
            );
        }
        if let Some(recovery) = current.recovery_fade
            && timeline_frame >= recovery.start_frame
            && timeline_frame
                < recovery
                    .start_frame
                    .saturating_add(recovery.duration_frames)
        {
            let progress = timeline_frame.saturating_sub(recovery.start_frame) as f32
                / recovery.duration_frames.max(1) as f32;
            gain *= recovery.start_gain + (1.0 - recovery.start_gain) * progress;
        }
        if let (Some(duration), Some((fade_frames, curve))) = (current.duration_frames, fade_out)
            && fade_frames > 0
            && timeline_frame >= duration.saturating_sub(fade_frames)
        {
            let remaining = duration.saturating_sub(timeline_frame);
            gain *= curve_gain(remaining as f32 / fade_frames as f32, curve);
        }
        if gain != 1.0 {
            for sample in frame {
                *sample *= gain;
            }
        }
    }
}

fn curve_gain(progress: f32, curve: GainCurve) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    match curve {
        GainCurve::Linear => progress,
        GainCurve::EqualPower => (progress * std::f32::consts::FRAC_PI_2).sin(),
    }
}

fn fail_promoted(
    actor: &mut ActorState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    message: String,
) {
    set_state(actor, PlaybackState::Failed, event_tx);
    let failure = PlaybackFailure::internal("sink", message).with_context(None, actor.generation);
    let _ = event_tx.send(PlaybackEvent::Failed(failure));
}

fn start_seek(
    actor: &mut ActorState,
    position: MediaTime,
) -> Result<(PlaybackItemId, DecoderSeekStatus), PlaybackControlError> {
    let current = actor
        .current
        .as_mut()
        .ok_or(PlaybackControlError::InvalidState)?;
    current.epoch = current.epoch.wrapping_add(1);
    current.pending_block = None;
    current.output.discard(current.epoch)?;
    current.sink_consumed_base_frame = 0;
    current.boundary_announced = true;
    let target = position
        .to_frames(current.decoded_format.sample_rate)
        .saturating_add(current.trim_head_frames);
    let status = current
        .decoder
        .start_seek(target)
        .map_err(|error| PlaybackControlError::failed("decoder", error.to_string()))?;
    Ok((current.item_id, status))
}

fn advance_pending_seek(event_tx: &broadcast::Sender<PlaybackEvent>, actor: &mut ActorState) {
    let Some(pending) = actor.pending_seek.take() else {
        return;
    };
    let status = actor
        .current
        .as_mut()
        .ok_or(PlaybackControlError::InvalidState)
        .and_then(|current| {
            current
                .decoder
                .continue_seek()
                .map_err(|error| PlaybackControlError::failed("decoder", error.to_string()))
        });
    match status {
        Ok(DecoderSeekStatus::Pending) => actor.pending_seek = Some(pending),
        Ok(DecoderSeekStatus::Complete(result)) => {
            finish_seek(actor, result, event_tx);
            set_state(actor, pending.resume_state, event_tx);
            let _ = event_tx.send(PlaybackEvent::Buffering {
                item_id: pending.item_id,
                active: false,
            });
            let _ = pending.response.send(Ok(CommandReply::Unit));
        },
        Err(error) => {
            set_state(actor, PlaybackState::Failed, event_tx);
            let _ = pending.response.send(Err(error));
        },
    }
}

fn finish_seek(
    actor: &mut ActorState,
    result: SeekResult,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    let Some(current) = actor.current.as_mut() else {
        return;
    };
    current.decoded_frame = result.actual_frame;
    let audible_frame = result.actual_frame.saturating_sub(current.trim_head_frames);
    current.produced_audible_frame = audible_frame;
    current.position_base_frame = audible_frame;
    current.last_reported_position_frame = audible_frame;
    current.tail_buffer.clear();
    current.drain_phase = DrainPhase::Decoding;
    current.fade_in_start_frame = audible_frame;
    current.fade_in_frames = current.seek_fade_frames;
    for transform in &mut current.pre_mix_transforms {
        transform.reset();
    }
    for transform in &mut current.post_mix_transforms {
        transform.reset();
    }
    if let Some(normalizer) = current.normalizer.as_mut() {
        normalizer.reset();
    }
    emit_position_if_due(current, event_tx, true);
}

fn trim_gapless_block(current: &mut ActiveTrack, block: &mut AudioBlock, raw_start: u64) {
    trim_gapless_samples(
        block,
        raw_start,
        current.trim_head_frames,
        current.trim_tail_frames,
        current.raw_duration_frames,
        &mut current.tail_buffer,
    );
}

fn trim_gapless_samples(
    block: &mut AudioBlock,
    raw_start: u64,
    trim_head_frames: u64,
    trim_tail_frames: u64,
    raw_duration_frames: Option<u64>,
    tail_buffer: &mut Vec<f32>,
) {
    let channels = usize::from(block.format.channels.max(1));
    let raw_end = raw_start.saturating_add(block.frames() as u64);
    let keep_start = raw_start.max(trim_head_frames);
    let known_keep_end =
        raw_duration_frames.map(|duration| duration.saturating_sub(trim_tail_frames));
    let keep_end = known_keep_end.map_or(raw_end, |end| raw_end.min(end));
    if keep_end <= keep_start {
        block.samples.clear();
        return;
    }
    let drop_head_frames = keep_start.saturating_sub(raw_start) as usize;
    let keep_frames = keep_end.saturating_sub(keep_start) as usize;
    let start_sample = drop_head_frames.saturating_mul(channels);
    let end_sample = start_sample.saturating_add(keep_frames.saturating_mul(channels));
    if start_sample > 0 || end_sample < block.samples.len() {
        block.samples = block.samples[start_sample..end_sample].to_vec();
    }

    if raw_duration_frames.is_none() && trim_tail_frames > 0 {
        tail_buffer.extend_from_slice(&block.samples);
        let held_samples = (trim_tail_frames as usize).saturating_mul(channels);
        if tail_buffer.len() <= held_samples {
            block.samples.clear();
        } else {
            let emit_samples = tail_buffer.len().saturating_sub(held_samples);
            block.samples = tail_buffer.drain(..emit_samples).collect();
        }
    }
}

fn emit_position_if_due(
    current: &mut ActiveTrack,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    force: bool,
) {
    let position_frame = current.position_base_frame.saturating_add(
        current
            .output
            .clock()
            .consumed_frames
            .saturating_sub(current.sink_consumed_base_frame),
    );
    if !current.boundary_announced {
        return;
    }
    let report_interval = u64::from(current.mix_format.sample_rate.max(1)) / 20;
    if !force
        && position_frame
            < current
                .last_reported_position_frame
                .saturating_add(report_interval.max(1))
    {
        return;
    }
    current.last_reported_position_frame = position_frame;
    let _ = event_tx.send(PlaybackEvent::Position {
        item_id: current.item_id,
        position: MediaTime::from_frames(position_frame, current.mix_format.sample_rate),
    });
}

fn set_state(
    actor: &mut ActorState,
    state: PlaybackState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    if actor.state != state {
        actor.state = state;
        let _ = event_tx.send(PlaybackEvent::StateChanged(state));
    }
}

fn publish_control_failure(
    error: &PlaybackControlError,
    event_tx: &broadcast::Sender<PlaybackEvent>,
) {
    if let PlaybackControlError::Failed(failure) = error {
        let _ = event_tx.send(PlaybackEvent::Failed(failure.clone()));
    }
}

fn fail_current(
    actor: &mut ActorState,
    event_tx: &broadcast::Sender<PlaybackEvent>,
    stage: &'static str,
    message: String,
) {
    let item_id = actor.current.as_ref().map(|current| current.item_id);
    stop_current(actor);
    set_state(actor, PlaybackState::Failed, event_tx);
    let failure = PlaybackFailure::internal(stage, message).with_context(item_id, actor.generation);
    let _ = event_tx.send(PlaybackEvent::Failed(failure));
}

fn reject_pending(actor: &mut ActorState) {
    if let Some(response) = actor.pending_current_response.take() {
        let _ = response.send(Err(PlaybackControlError::Closed));
    }
    if let Some(response) = actor.pending_next_response.take() {
        let _ = response.send(Err(PlaybackControlError::Closed));
    }
    if let Some(pending) = actor.pending_seek.take() {
        let _ = pending.response.send(Err(PlaybackControlError::Closed));
    }
}

fn stop_current(actor: &mut ActorState) {
    if let Some(mut current) = actor.current.take() {
        current.decoder.reset();
        for transform in &mut current.pre_mix_transforms {
            transform.reset();
        }
        for transform in &mut current.post_mix_transforms {
            transform.reset();
        }
        if let Some(normalizer) = current.normalizer.as_mut() {
            normalizer.reset();
        }
        current.output.shutdown();
    }
}

enum PendingWrite {
    Full(AudioBlock),
    Closed,
}

enum SinkDataCommand {
    Write(AudioBlock),
    Boundary(PlaybackItemId),
}

enum SinkControlCommand {
    Pause(std::sync::mpsc::Sender<Result<(), String>>),
    Resume(std::sync::mpsc::Sender<Result<(), String>>),
    Discard {
        epoch: u64,
        response: std::sync::mpsc::Sender<Result<(), String>>,
    },
    SetGain {
        target: f32,
        duration_frames: u64,
    },
    Drain(std::sync::mpsc::Sender<Result<(), String>>),
    Shutdown,
}

struct SinkWorkerClock {
    consumed_frames: AtomicU64,
    buffered_frames: AtomicU64,
    device_buffered_frames: AtomicU64,
    epoch: AtomicU64,
}

struct SinkWorker {
    data_sender: Sender<SinkDataCommand>,
    control_sender: Sender<SinkControlCommand>,
    boundary_receiver: Receiver<PlaybackItemId>,
    failure_receiver: Receiver<String>,
    clock: Arc<SinkWorkerClock>,
    join: Option<JoinHandle<()>>,
}

impl SinkWorker {
    fn start(
        factory: Arc<dyn SinkFactory>,
        format: AudioFormat,
        capacity: usize,
        initial_gain: f32,
    ) -> Result<Self, PlaybackControlError> {
        let (data_sender, data_receiver) = crossbeam_channel::bounded(capacity.max(1));
        let (control_sender, control_receiver) = crossbeam_channel::bounded(16);
        let (boundary_sender, boundary_receiver) = crossbeam_channel::unbounded();
        let (failure_sender, failure_receiver) = crossbeam_channel::bounded(1);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let clock = Arc::new(SinkWorkerClock {
            consumed_frames: AtomicU64::new(0),
            buffered_frames: AtomicU64::new(0),
            device_buffered_frames: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
        });
        let worker_clock = Arc::clone(&clock);
        let join = std::thread::Builder::new()
            .name("stellatune-sink-worker".to_owned())
            .spawn(move || {
                let mut sink = match factory.create() {
                    Ok(sink) => sink,
                    Err(error) => {
                        let _ = started_tx.send(Err(error.to_string()));
                        return;
                    },
                };
                if let Err(error) = sink.open(format) {
                    let _ = started_tx.send(Err(error.to_string()));
                    return;
                }
                let _ = sink.pause();
                let _ = started_tx.send(Ok(()));
                sink_worker_loop(
                    sink,
                    data_receiver,
                    control_receiver,
                    worker_clock,
                    boundary_sender,
                    failure_sender,
                    initial_gain,
                );
            })
            .map_err(|error| PlaybackControlError::failed("sink", error.to_string()))?;
        started_rx
            .recv()
            .map_err(|_| {
                PlaybackControlError::failed("sink", "sink worker exited during startup".to_owned())
            })?
            .map_err(|message| PlaybackControlError::failed("sink", message))?;
        Ok(Self {
            data_sender,
            control_sender,
            boundary_receiver,
            failure_receiver,
            clock,
            join: Some(join),
        })
    }

    fn try_write(&self, block: AudioBlock) -> Result<(), PendingWrite> {
        self.clock
            .buffered_frames
            .fetch_add(block.frames() as u64, Ordering::Relaxed);
        match self.data_sender.try_send(SinkDataCommand::Write(block)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(SinkDataCommand::Write(block))) => {
                self.clock
                    .buffered_frames
                    .fetch_sub(block.frames() as u64, Ordering::Relaxed);
                Err(PendingWrite::Full(block))
            },
            Err(TrySendError::Disconnected(SinkDataCommand::Write(block))) => {
                self.clock
                    .buffered_frames
                    .fetch_sub(block.frames() as u64, Ordering::Relaxed);
                Err(PendingWrite::Closed)
            },
            Err(_) => unreachable!("try_write only sends Write"),
        }
    }

    fn control(
        &self,
        build: impl FnOnce(std::sync::mpsc::Sender<Result<(), String>>) -> SinkControlCommand,
    ) -> Result<(), PlaybackControlError> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.control_sender
            .send(build(tx))
            .map_err(|_| PlaybackControlError::Closed)?;
        rx.recv()
            .map_err(|_| PlaybackControlError::Closed)?
            .map_err(|message| PlaybackControlError::failed("sink", message))
    }

    fn pause(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Pause)
    }

    fn resume(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Resume)
    }

    fn discard(&self, epoch: u64) -> Result<(), PlaybackControlError> {
        self.control(|response| SinkControlCommand::Discard { epoch, response })
    }

    fn set_gain(&self, target: f32, duration_frames: u64) -> Result<(), PlaybackControlError> {
        self.control_sender
            .send(SinkControlCommand::SetGain {
                target,
                duration_frames,
            })
            .map_err(|_| PlaybackControlError::Closed)
    }

    fn drain(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Drain)
    }

    fn mark_boundary(&self, item_id: PlaybackItemId) -> Result<(), PlaybackControlError> {
        self.data_sender
            .try_send(SinkDataCommand::Boundary(item_id))
            .map_err(|error| match error {
                TrySendError::Disconnected(_) => PlaybackControlError::Closed,
                TrySendError::Full(_) => PlaybackControlError::failed(
                    "sink",
                    "PCM ring is full while inserting an item boundary".to_owned(),
                ),
            })
    }

    fn try_boundary(&self) -> Option<PlaybackItemId> {
        self.boundary_receiver.try_recv().ok()
    }

    fn try_failure(&self) -> Option<String> {
        self.failure_receiver.try_recv().ok()
    }

    fn clock(&self) -> SinkClockSnapshot {
        SinkClockSnapshot {
            consumed_frames: self.clock.consumed_frames.load(Ordering::Relaxed),
            buffered_frames: self
                .clock
                .buffered_frames
                .load(Ordering::Relaxed)
                .saturating_add(self.clock.device_buffered_frames.load(Ordering::Relaxed)),
            epoch: self.clock.epoch.load(Ordering::Relaxed),
        }
    }

    fn shutdown(&mut self) {
        let _ = self.control_sender.send(SinkControlCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for SinkWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct OutputGainEnvelope {
    start: f32,
    current: f32,
    target: f32,
    progressed_frames: u64,
    duration_frames: u64,
}

impl OutputGainEnvelope {
    fn new(initial: f32) -> Self {
        let initial = initial.clamp(0.0, 1.0);
        Self {
            start: initial,
            current: initial,
            target: initial,
            progressed_frames: 0,
            duration_frames: 0,
        }
    }

    fn schedule(&mut self, target: f32, duration_frames: u64) {
        self.start = self.current;
        self.target = target.clamp(0.0, 1.0);
        self.progressed_frames = 0;
        self.duration_frames = duration_frames;
        if duration_frames == 0 {
            self.current = self.target;
        }
    }

    fn apply(&mut self, block: &mut AudioBlock) {
        let channels = usize::from(block.format.channels.max(1));
        for frame in block.samples.chunks_exact_mut(channels) {
            if self.progressed_frames < self.duration_frames {
                self.progressed_frames = self.progressed_frames.saturating_add(1);
                let progress = self.progressed_frames as f32 / self.duration_frames.max(1) as f32;
                self.current = self.start + (self.target - self.start) * progress.clamp(0.0, 1.0);
            } else {
                self.current = self.target;
            }
            if self.current != 1.0 {
                for sample in frame {
                    *sample *= self.current;
                }
            }
        }
    }
}

fn sink_worker_loop(
    mut sink: Box<dyn SinkStage>,
    data_receiver: Receiver<SinkDataCommand>,
    control_receiver: Receiver<SinkControlCommand>,
    clock: Arc<SinkWorkerClock>,
    boundary_sender: Sender<PlaybackItemId>,
    failure_sender: Sender<String>,
    initial_gain: f32,
) {
    let mut gain = OutputGainEnvelope::new(initial_gain);
    let mut accepted_frames = 0_u64;
    let mut pending_boundaries = VecDeque::<(u64, PlaybackItemId)>::new();
    let mut pending_block = None::<AudioBlock>;
    let mut pending_drain = None::<std::sync::mpsc::Sender<Result<(), String>>>;
    let mut paused = true;
    'worker: loop {
        while let Ok(command) = control_receiver.try_recv() {
            if handle_sink_control(
                command,
                sink.as_mut(),
                &data_receiver,
                clock.as_ref(),
                &mut gain,
                &mut paused,
                &mut pending_block,
                &mut pending_drain,
                &mut accepted_frames,
                &mut pending_boundaries,
            ) {
                break 'worker;
            }
        }

        if pending_drain.is_some()
            && pending_block.is_none()
            && data_receiver.is_empty()
            && let Some(response) = pending_drain.take()
        {
            let _ = response.send(sink.drain().map_err(|error| error.to_string()));
        }

        if !paused {
            if let Some(block) = pending_block.as_mut() {
                match sink.write(block) {
                    Ok(result) => {
                        let consumed = result.consumed_frames.min(block.frames());
                        if consumed > 0 {
                            accepted_frames = accepted_frames.saturating_add(consumed as u64);
                            clock
                                .buffered_frames
                                .fetch_sub(consumed as u64, Ordering::Relaxed);
                            let samples =
                                consumed.saturating_mul(usize::from(block.format.channels));
                            block.samples.drain(..samples);
                            block.timeline.start_frame =
                                block.timeline.start_frame.saturating_add(consumed as u64);
                        }
                        if block.samples.is_empty() {
                            pending_block = None;
                        } else if result.state == SinkWriteState::WouldBlock || consumed == 0 {
                            std::thread::yield_now();
                        }
                    },
                    Err(error) => {
                        clock
                            .buffered_frames
                            .fetch_sub(block.frames() as u64, Ordering::Relaxed);
                        let _ = failure_sender.try_send(error.to_string());
                        break 'worker;
                    },
                }
            } else {
                crossbeam_channel::select_biased! {
                    recv(control_receiver) -> command => match command {
                        Ok(command) => {
                            if handle_sink_control(
                                command,
                                sink.as_mut(),
                                &data_receiver,
                                clock.as_ref(),
                                &mut gain,
                                &mut paused,
                                &mut pending_block,
                                &mut pending_drain,
                                &mut accepted_frames,
                                &mut pending_boundaries,
                            ) {
                                break 'worker;
                            }
                        },
                        Err(_) => break 'worker,
                    },
                    recv(data_receiver) -> command => match command {
                        Ok(command) => accept_sink_data(
                            command,
                            clock.as_ref(),
                            &mut gain,
                            &mut pending_block,
                            accepted_frames,
                            &mut pending_boundaries,
                        ),
                        Err(_) => break 'worker,
                    },
                    default(Duration::from_millis(2)) => {},
                }
            }
        } else {
            match control_receiver.recv_timeout(Duration::from_millis(2)) {
                Ok(command) => {
                    if handle_sink_control(
                        command,
                        sink.as_mut(),
                        &data_receiver,
                        clock.as_ref(),
                        &mut gain,
                        &mut paused,
                        &mut pending_block,
                        &mut pending_drain,
                        &mut accepted_frames,
                        &mut pending_boundaries,
                    ) {
                        break;
                    }
                },
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {},
            }
        }
        let snapshot = sync_sink_clock(sink.as_ref(), clock.as_ref());
        publish_consumed_boundaries(
            snapshot.consumed_frames,
            &mut pending_boundaries,
            &boundary_sender,
        );
    }
    sink.close();
}

fn accept_sink_data(
    command: SinkDataCommand,
    clock: &SinkWorkerClock,
    gain: &mut OutputGainEnvelope,
    pending_block: &mut Option<AudioBlock>,
    accepted_frames: u64,
    pending_boundaries: &mut VecDeque<(u64, PlaybackItemId)>,
) {
    match command {
        SinkDataCommand::Write(mut block) => {
            if block.timeline.epoch != clock.epoch.load(Ordering::Relaxed) {
                clock
                    .buffered_frames
                    .fetch_sub(block.frames() as u64, Ordering::Relaxed);
                return;
            }
            gain.apply(&mut block);
            *pending_block = Some(block);
        },
        SinkDataCommand::Boundary(item_id) => {
            pending_boundaries.push_back((accepted_frames, item_id));
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_sink_control(
    command: SinkControlCommand,
    sink: &mut dyn SinkStage,
    data_receiver: &Receiver<SinkDataCommand>,
    clock: &SinkWorkerClock,
    gain: &mut OutputGainEnvelope,
    paused: &mut bool,
    pending_block: &mut Option<AudioBlock>,
    pending_drain: &mut Option<std::sync::mpsc::Sender<Result<(), String>>>,
    accepted_frames: &mut u64,
    pending_boundaries: &mut VecDeque<(u64, PlaybackItemId)>,
) -> bool {
    match command {
        SinkControlCommand::Pause(response) => {
            let result = sink.pause().map_err(|error| error.to_string());
            if result.is_ok() {
                *paused = true;
            }
            let _ = response.send(result);
        },
        SinkControlCommand::Resume(response) => {
            let result = sink.resume().map_err(|error| error.to_string());
            if result.is_ok() {
                *paused = false;
            }
            let _ = response.send(result);
        },
        SinkControlCommand::Discard { epoch, response } => {
            let result = sink.discard().map_err(|error| error.to_string());
            *pending_block = None;
            while data_receiver.try_recv().is_ok() {}
            if let Some(drain) = pending_drain.take() {
                let _ = drain.send(Err("drain canceled by discard".to_owned()));
            }
            clock.consumed_frames.store(0, Ordering::Relaxed);
            clock.buffered_frames.store(0, Ordering::Relaxed);
            clock.device_buffered_frames.store(0, Ordering::Relaxed);
            clock.epoch.store(epoch, Ordering::Relaxed);
            *accepted_frames = 0;
            pending_boundaries.clear();
            let _ = response.send(result);
        },
        SinkControlCommand::SetGain {
            target,
            duration_frames,
        } => gain.schedule(target, duration_frames),
        SinkControlCommand::Drain(response) => {
            if let Some(replaced) = pending_drain.replace(response) {
                let _ = replaced.send(Err("drain was superseded".to_owned()));
            }
        },
        SinkControlCommand::Shutdown => return true,
    }
    false
}

fn sync_sink_clock(sink: &dyn SinkStage, clock: &SinkWorkerClock) -> SinkClockSnapshot {
    let snapshot = sink.clock_snapshot();
    clock
        .consumed_frames
        .store(snapshot.consumed_frames, Ordering::Relaxed);
    clock
        .device_buffered_frames
        .store(snapshot.buffered_frames, Ordering::Relaxed);
    snapshot
}

fn publish_consumed_boundaries(
    consumed_frames: u64,
    pending: &mut VecDeque<(u64, PlaybackItemId)>,
    sender: &Sender<PlaybackItemId>,
) {
    while pending
        .front()
        .is_some_and(|(target, _)| consumed_frames >= *target)
    {
        if let Some((_, item_id)) = pending.pop_front() {
            let _ = sender.send(item_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use stellatune_audio_core::{
        DecodeError, DecodedStreamInfo, DecoderDescriptor, DecoderFactory, DrainStatus,
        FactoryError, MediaHints, MemorySourceFactory, OutputCompatibilityKey, SinkError,
        SinkWriteResult, SourceCapabilities, SourceDescriptor, SourceFactory, StageId,
        TransformDescriptor, TransformError, TransformFactory, TransformPlacement,
    };
    use tokio::time::{Duration, timeout};

    use super::*;
    use crate::planner::{CrossfadeCurve, CrossfadeFallback, TransitionPolicy};

    struct TestDecoderFactory {
        descriptor: DecoderDescriptor,
    }

    struct CountingTransformFactory {
        descriptor: TransformDescriptor,
        process_counts: Arc<Mutex<Vec<usize>>>,
    }

    impl CountingTransformFactory {
        fn new(
            id: &str,
            placement: TransformPlacement,
            process_counts: Arc<Mutex<Vec<usize>>>,
        ) -> Self {
            Self {
                descriptor: TransformDescriptor {
                    id: StageId::new(id).unwrap(),
                    placement,
                },
                process_counts,
            }
        }
    }

    impl TransformFactory for CountingTransformFactory {
        fn descriptor(&self) -> &TransformDescriptor {
            &self.descriptor
        }

        fn create(&self) -> Result<Box<dyn TransformStage>, FactoryError> {
            let index = {
                let mut counts = self.process_counts.lock().unwrap();
                counts.push(0);
                counts.len() - 1
            };
            Ok(Box::new(CountingTransform {
                index,
                process_counts: Arc::clone(&self.process_counts),
            }))
        }
    }

    struct CountingTransform {
        index: usize,
        process_counts: Arc<Mutex<Vec<usize>>>,
    }

    struct BufferingTailTransformFactory {
        descriptor: TransformDescriptor,
    }

    impl BufferingTailTransformFactory {
        fn new(placement: TransformPlacement) -> Self {
            Self {
                descriptor: TransformDescriptor {
                    id: StageId::new(match placement {
                        TransformPlacement::PreMix => "test.buffering-pre",
                        TransformPlacement::PostMix => "test.buffering-post",
                    })
                    .unwrap(),
                    placement,
                },
            }
        }
    }

    impl TransformFactory for BufferingTailTransformFactory {
        fn descriptor(&self) -> &TransformDescriptor {
            &self.descriptor
        }

        fn create(&self) -> Result<Box<dyn TransformStage>, FactoryError> {
            Ok(Box::new(BufferingTailTransform { buffered: None }))
        }
    }

    struct BufferingTailTransform {
        buffered: Option<Vec<f32>>,
    }

    impl TransformStage for BufferingTailTransform {
        fn configure(&mut self, input: AudioFormat) -> Result<AudioFormat, TransformError> {
            Ok(input)
        }

        fn process(&mut self, block: &mut AudioBlock) -> Result<TransformStatus, TransformError> {
            self.buffered = Some(block.samples.clone());
            Ok(TransformStatus::Buffered)
        }

        fn drain(&mut self, output: &mut AudioBlock) -> Result<DrainStatus, TransformError> {
            match self.buffered.take() {
                Some(samples) => {
                    output.samples = samples;
                    Ok(DrainStatus::Produced)
                },
                None => Ok(DrainStatus::Complete),
            }
        }

        fn reset(&mut self) {
            self.buffered = None;
        }
    }

    impl TransformStage for CountingTransform {
        fn configure(&mut self, input: AudioFormat) -> Result<AudioFormat, TransformError> {
            Ok(input)
        }

        fn process(&mut self, _block: &mut AudioBlock) -> Result<TransformStatus, TransformError> {
            self.process_counts.lock().unwrap()[self.index] += 1;
            Ok(TransformStatus::Produced)
        }

        fn drain(&mut self, _output: &mut AudioBlock) -> Result<DrainStatus, TransformError> {
            Ok(DrainStatus::Complete)
        }

        fn reset(&mut self) {}
    }

    impl TestDecoderFactory {
        fn new() -> Self {
            Self {
                descriptor: DecoderDescriptor {
                    id: StageId::new("test.decoder").unwrap(),
                    priority: 1,
                    extensions: Vec::new(),
                    mime_types: Vec::new(),
                },
            }
        }
    }

    impl DecoderFactory for TestDecoderFactory {
        fn descriptor(&self) -> &DecoderDescriptor {
            &self.descriptor
        }

        fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
            Ok(Box::new(TestDecoder {
                remaining: 0,
                total: 0,
                amplitude: 0.0,
            }))
        }
    }

    struct TestDecoder {
        remaining: u64,
        total: u64,
        amplitude: f32,
    }

    struct FixedFormatDecoderFactory {
        descriptor: DecoderDescriptor,
        format: AudioFormat,
        frames: u64,
        amplitude: f32,
    }

    impl FixedFormatDecoderFactory {
        fn new(id: &str, format: AudioFormat, frames: u64, amplitude: f32) -> Self {
            Self {
                descriptor: DecoderDescriptor {
                    id: StageId::new(id).unwrap(),
                    priority: 10,
                    extensions: Vec::new(),
                    mime_types: Vec::new(),
                },
                format,
                frames,
                amplitude,
            }
        }
    }

    impl DecoderFactory for FixedFormatDecoderFactory {
        fn descriptor(&self) -> &DecoderDescriptor {
            &self.descriptor
        }

        fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
            Ok(Box::new(FixedFormatDecoder {
                format: self.format,
                total: self.frames,
                remaining: self.frames,
                amplitude: self.amplitude,
            }))
        }
    }

    struct FixedFormatDecoder {
        format: AudioFormat,
        total: u64,
        remaining: u64,
        amplitude: f32,
    }

    struct FailingNextDecoderFactory {
        descriptor: DecoderDescriptor,
    }

    impl FailingNextDecoderFactory {
        fn new() -> Self {
            Self {
                descriptor: DecoderDescriptor {
                    id: StageId::new("test.failing-next").unwrap(),
                    priority: 10,
                    extensions: Vec::new(),
                    mime_types: Vec::new(),
                },
            }
        }
    }

    impl DecoderFactory for FailingNextDecoderFactory {
        fn descriptor(&self) -> &DecoderDescriptor {
            &self.descriptor
        }

        fn create(&self) -> Result<Box<dyn DecoderStage>, FactoryError> {
            Ok(Box::new(FailingNextDecoder { emitted: false }))
        }
    }

    struct FailingNextDecoder {
        emitted: bool,
    }

    impl DecoderStage for FailingNextDecoder {
        fn open(
            &mut self,
            _source: Box<dyn stellatune_audio_core::EncodedSource>,
            _hints: &MediaHints,
        ) -> Result<DecodedStreamInfo, DecodeError> {
            Ok(DecodedStreamInfo {
                format: AudioFormat {
                    sample_rate: 100,
                    channels: 1,
                    channel_mask: None,
                },
                duration_frames: Some(100),
                gapless_trim: None,
            })
        }

        fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
            if self.emitted {
                return Err(DecodeError::Failed {
                    message: "simulated next decoder failure".to_owned(),
                });
            }
            self.emitted = true;
            output.samples = vec![0.0; 10];
            Ok(DecodeStatus::Produced { frames: 10 })
        }

        fn start_seek(&mut self, _target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
            Err(DecodeError::Unsupported)
        }

        fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
            Err(DecodeError::Unsupported)
        }

        fn reset(&mut self) {}
    }

    impl DecoderStage for FixedFormatDecoder {
        fn open(
            &mut self,
            _source: Box<dyn stellatune_audio_core::EncodedSource>,
            _hints: &MediaHints,
        ) -> Result<DecodedStreamInfo, DecodeError> {
            Ok(DecodedStreamInfo {
                format: self.format,
                duration_frames: Some(self.total),
                gapless_trim: None,
            })
        }

        fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
            if self.remaining == 0 {
                return Ok(DecodeStatus::EndOfStream);
            }
            let frames = self.remaining.min(64) as usize;
            output.samples.resize(
                frames.saturating_mul(usize::from(self.format.channels)),
                self.amplitude,
            );
            self.remaining -= frames as u64;
            Ok(DecodeStatus::Produced { frames })
        }

        fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
            let actual = target_frame.min(self.total);
            self.remaining = self.total.saturating_sub(actual);
            Ok(DecoderSeekStatus::Complete(SeekResult {
                actual_frame: actual,
            }))
        }

        fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
            Err(DecodeError::Unsupported)
        }

        fn reset(&mut self) {}
    }

    impl DecoderStage for TestDecoder {
        fn open(
            &mut self,
            mut source: Box<dyn stellatune_audio_core::EncodedSource>,
            _hints: &MediaHints,
        ) -> Result<DecodedStreamInfo, DecodeError> {
            let mut input = [0_u8; 2];
            source.read_exact(&mut input).map_err(DecodeError::Io)?;
            self.total = u64::from(input[0]);
            self.remaining = self.total;
            self.amplitude = f32::from(input[1]) / 100.0;
            Ok(DecodedStreamInfo {
                format: AudioFormat {
                    sample_rate: 100,
                    channels: 1,
                    channel_mask: None,
                },
                duration_frames: Some(self.total),
                gapless_trim: None,
            })
        }

        fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
            if self.remaining == 0 {
                return Ok(DecodeStatus::EndOfStream);
            }
            let frames = self.remaining.min(10) as usize;
            output.samples.resize(frames, self.amplitude);
            self.remaining -= frames as u64;
            Ok(DecodeStatus::Produced { frames })
        }

        fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
            self.remaining = self.total.saturating_sub(target_frame);
            Ok(DecoderSeekStatus::Complete(SeekResult {
                actual_frame: target_frame.min(self.total),
            }))
        }

        fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
            Err(DecodeError::Unsupported)
        }

        fn reset(&mut self) {}
    }

    struct TestSinkFactory {
        id: StageId,
        samples: Arc<Mutex<Vec<f32>>>,
    }

    impl SinkFactory for TestSinkFactory {
        fn id(&self) -> &StageId {
            &self.id
        }

        fn compatibility_key(
            &self,
            format: AudioFormat,
        ) -> Result<OutputCompatibilityKey, FactoryError> {
            Ok(OutputCompatibilityKey {
                backend_id: "test".to_owned(),
                device_id: None,
                sample_rate: format.sample_rate,
                channels: format.channels,
                route_revision: 0,
            })
        }

        fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
            Ok(Box::new(TestSink {
                samples: Arc::clone(&self.samples),
                consumed: 0,
                epoch: 0,
            }))
        }
    }

    struct TestSink {
        samples: Arc<Mutex<Vec<f32>>>,
        consumed: u64,
        epoch: u64,
    }

    struct RecordingSinkFactory {
        id: StageId,
        formats: Arc<Mutex<Vec<AudioFormat>>>,
        creates: Arc<AtomicUsize>,
    }

    struct FormatAdaptingSinkFactory {
        id: StageId,
        target: AudioFormat,
        formats: Arc<Mutex<Vec<AudioFormat>>>,
        samples: Arc<Mutex<Vec<f32>>>,
    }

    impl SinkFactory for FormatAdaptingSinkFactory {
        fn id(&self) -> &StageId {
            &self.id
        }

        fn preferred_format(&self, _input: AudioFormat) -> Result<AudioFormat, FactoryError> {
            Ok(self.target)
        }

        fn compatibility_key(
            &self,
            format: AudioFormat,
        ) -> Result<OutputCompatibilityKey, FactoryError> {
            Ok(OutputCompatibilityKey {
                backend_id: "format-adapting".to_owned(),
                device_id: None,
                sample_rate: format.sample_rate,
                channels: format.channels,
                route_revision: 0,
            })
        }

        fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
            Ok(Box::new(FormatAdaptingSink {
                formats: Arc::clone(&self.formats),
                samples: Arc::clone(&self.samples),
                consumed: 0,
            }))
        }
    }

    struct FormatAdaptingSink {
        formats: Arc<Mutex<Vec<AudioFormat>>>,
        samples: Arc<Mutex<Vec<f32>>>,
        consumed: u64,
    }

    impl SinkStage for FormatAdaptingSink {
        fn open(&mut self, format: AudioFormat) -> Result<(), SinkError> {
            self.formats.lock().unwrap().push(format);
            Ok(())
        }

        fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
            assert_eq!(block.format, *self.formats.lock().unwrap().last().unwrap());
            self.samples
                .lock()
                .unwrap()
                .extend_from_slice(&block.samples);
            self.consumed = self.consumed.saturating_add(block.frames() as u64);
            Ok(SinkWriteResult {
                consumed_frames: block.frames(),
                state: SinkWriteState::Ready,
            })
        }

        fn pause(&mut self) -> Result<(), SinkError> {
            Ok(())
        }

        fn resume(&mut self) -> Result<(), SinkError> {
            Ok(())
        }

        fn drain(&mut self) -> Result<(), SinkError> {
            Ok(())
        }

        fn discard(&mut self) -> Result<(), SinkError> {
            self.consumed = 0;
            Ok(())
        }

        fn clock_snapshot(&self) -> SinkClockSnapshot {
            SinkClockSnapshot {
                consumed_frames: self.consumed,
                buffered_frames: 0,
                epoch: 0,
            }
        }

        fn close(&mut self) {}
    }

    impl SinkFactory for RecordingSinkFactory {
        fn id(&self) -> &StageId {
            &self.id
        }

        fn compatibility_key(
            &self,
            format: AudioFormat,
        ) -> Result<OutputCompatibilityKey, FactoryError> {
            Ok(OutputCompatibilityKey {
                backend_id: "recording".to_owned(),
                device_id: None,
                sample_rate: format.sample_rate,
                channels: format.channels,
                route_revision: 0,
            })
        }

        fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
            self.creates.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(RecordingSink {
                formats: Arc::clone(&self.formats),
                consumed: 0,
            }))
        }
    }

    struct RecordingSink {
        formats: Arc<Mutex<Vec<AudioFormat>>>,
        consumed: u64,
    }

    struct StalledSinkFactory {
        id: StageId,
        allow_writes: Arc<std::sync::atomic::AtomicBool>,
    }

    impl SinkFactory for StalledSinkFactory {
        fn id(&self) -> &StageId {
            &self.id
        }

        fn compatibility_key(
            &self,
            format: AudioFormat,
        ) -> Result<OutputCompatibilityKey, FactoryError> {
            Ok(OutputCompatibilityKey {
                backend_id: "stalled".to_owned(),
                device_id: None,
                sample_rate: format.sample_rate,
                channels: format.channels,
                route_revision: 0,
            })
        }

        fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
            Ok(Box::new(StalledSink {
                allow_writes: Arc::clone(&self.allow_writes),
                consumed: 0,
            }))
        }
    }

    struct StalledSink {
        allow_writes: Arc<std::sync::atomic::AtomicBool>,
        consumed: u64,
    }

    impl SinkStage for StalledSink {
        fn open(&mut self, _format: AudioFormat) -> Result<(), SinkError> {
            Ok(())
        }
        fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
            if !self.allow_writes.load(Ordering::SeqCst) {
                return Ok(SinkWriteResult {
                    consumed_frames: 0,
                    state: SinkWriteState::WouldBlock,
                });
            }
            self.consumed = self.consumed.saturating_add(block.frames() as u64);
            Ok(SinkWriteResult {
                consumed_frames: block.frames(),
                state: SinkWriteState::Ready,
            })
        }
        fn pause(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn drain(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn discard(&mut self) -> Result<(), SinkError> {
            self.consumed = 0;
            Ok(())
        }
        fn clock_snapshot(&self) -> SinkClockSnapshot {
            SinkClockSnapshot {
                consumed_frames: self.consumed,
                buffered_frames: 0,
                epoch: 0,
            }
        }
        fn close(&mut self) {}
    }

    impl SinkStage for RecordingSink {
        fn open(&mut self, _format: AudioFormat) -> Result<(), SinkError> {
            Ok(())
        }
        fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
            self.formats.lock().unwrap().push(block.format);
            self.consumed = self.consumed.saturating_add(block.frames() as u64);
            Ok(SinkWriteResult {
                consumed_frames: block.frames(),
                state: SinkWriteState::Ready,
            })
        }
        fn pause(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn drain(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn discard(&mut self) -> Result<(), SinkError> {
            self.consumed = 0;
            Ok(())
        }
        fn clock_snapshot(&self) -> SinkClockSnapshot {
            SinkClockSnapshot {
                consumed_frames: self.consumed,
                buffered_frames: 0,
                epoch: 0,
            }
        }
        fn close(&mut self) {}
    }

    struct RecoveringSinkFactory {
        id: StageId,
        samples: Arc<Mutex<Vec<f32>>>,
        creates: Arc<AtomicUsize>,
    }

    #[derive(Clone)]
    struct DelayedSourceFactory {
        inner: MemorySourceFactory,
        delay: Duration,
    }

    impl SourceFactory for DelayedSourceFactory {
        fn descriptor(&self) -> SourceDescriptor {
            self.inner.descriptor()
        }

        fn open(&self, request: SourceOpenRequest) -> stellatune_audio_core::SourceOpenFuture<'_> {
            let inner = self.inner.clone();
            let delay = self.delay;
            let cancellation = request.cancellation.clone();
            Box::pin(async move {
                tokio::select! {
                    () = tokio::time::sleep(delay) => inner.open(request).await,
                    () = cancellation.cancelled() => Err(stellatune_audio_core::SourceError::Cancelled),
                }
            })
        }
    }

    impl SinkFactory for RecoveringSinkFactory {
        fn id(&self) -> &StageId {
            &self.id
        }

        fn compatibility_key(
            &self,
            format: AudioFormat,
        ) -> Result<OutputCompatibilityKey, FactoryError> {
            Ok(OutputCompatibilityKey {
                backend_id: "recovering-test".to_owned(),
                device_id: None,
                sample_rate: format.sample_rate,
                channels: format.channels,
                route_revision: 0,
            })
        }

        fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
            let instance = self.creates.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(RecoveringSink {
                samples: Arc::clone(&self.samples),
                consumed: 0,
                writes: 0,
                fail_on_second_write: instance == 0,
            }))
        }
    }

    struct RecoveringSink {
        samples: Arc<Mutex<Vec<f32>>>,
        consumed: u64,
        writes: usize,
        fail_on_second_write: bool,
    }

    impl SinkStage for RecoveringSink {
        fn open(&mut self, _format: AudioFormat) -> Result<(), SinkError> {
            Ok(())
        }

        fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
            self.writes += 1;
            if self.fail_on_second_write && self.writes == 2 {
                return Err(SinkError::Failed {
                    message: "simulated device disconnect".to_owned(),
                });
            }
            self.samples
                .lock()
                .unwrap()
                .extend_from_slice(&block.samples);
            self.consumed = self.consumed.saturating_add(block.frames() as u64);
            Ok(SinkWriteResult {
                consumed_frames: block.frames(),
                state: SinkWriteState::Ready,
            })
        }

        fn pause(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn drain(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn discard(&mut self) -> Result<(), SinkError> {
            self.consumed = 0;
            Ok(())
        }
        fn clock_snapshot(&self) -> SinkClockSnapshot {
            SinkClockSnapshot {
                consumed_frames: self.consumed,
                buffered_frames: 0,
                epoch: 0,
            }
        }
        fn close(&mut self) {}
    }

    impl SinkStage for TestSink {
        fn open(&mut self, _format: AudioFormat) -> Result<(), SinkError> {
            Ok(())
        }

        fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
            self.samples
                .lock()
                .unwrap()
                .extend_from_slice(&block.samples);
            self.consumed = self.consumed.saturating_add(block.frames() as u64);
            Ok(SinkWriteResult {
                consumed_frames: block.frames(),
                state: SinkWriteState::Ready,
            })
        }

        fn pause(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn drain(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn discard(&mut self) -> Result<(), SinkError> {
            self.consumed = 0;
            self.epoch = self.epoch.wrapping_add(1);
            Ok(())
        }
        fn clock_snapshot(&self) -> SinkClockSnapshot {
            SinkClockSnapshot {
                consumed_frames: self.consumed,
                buffered_frames: 0,
                epoch: self.epoch,
            }
        }
        fn close(&mut self) {}
    }

    fn item(id: u64, frames: u8, amplitude: u8) -> PlaybackItem {
        PlaybackItem {
            id: PlaybackItemId::new(id).unwrap(),
            source: Arc::new(MemorySourceFactory::new(
                Arc::<[u8]>::from([frames, amplitude]),
                SourceDescriptor {
                    media: MediaHints::default(),
                    capabilities: SourceCapabilities {
                        byte_seekable: true,
                        reopenable: true,
                        live: false,
                    },
                },
            )),
            required_decoder: None,
        }
    }

    fn fixed_format_item(id: u64, factory: Arc<dyn DecoderFactory>) -> PlaybackItem {
        PlaybackItem {
            id: PlaybackItemId::new(id).unwrap(),
            source: Arc::new(MemorySourceFactory::new(
                Arc::<[u8]>::from([0_u8]),
                SourceDescriptor {
                    media: MediaHints::default(),
                    capabilities: SourceCapabilities {
                        byte_seekable: true,
                        reopenable: true,
                        live: false,
                    },
                },
            )),
            required_decoder: Some(factory),
        }
    }

    fn delayed_item(id: u64, frames: u8, amplitude: u8, delay: Duration) -> PlaybackItem {
        PlaybackItem {
            id: PlaybackItemId::new(id).unwrap(),
            source: Arc::new(DelayedSourceFactory {
                inner: MemorySourceFactory::new(
                    Arc::<[u8]>::from([frames, amplitude]),
                    SourceDescriptor {
                        media: MediaHints::default(),
                        capabilities: SourceCapabilities {
                            byte_seekable: true,
                            reopenable: true,
                            live: false,
                        },
                    },
                ),
                delay,
            }),
            required_decoder: None,
        }
    }

    fn runtime(transition: TransitionPolicy, samples: Arc<Mutex<Vec<f32>>>) -> PlaybackRuntime {
        let registry = StageRegistrySnapshot {
            decoders: vec![Arc::new(TestDecoderFactory::new())],
            transforms: Vec::new(),
            sink: Arc::new(TestSinkFactory {
                id: StageId::new("test.sink").unwrap(),
                samples,
            }),
        };
        let mut config = PlaybackRuntimeConfig::new(registry);
        config.block_frames = 10;
        config.pcm_ring_blocks = 2;
        config.policies.transition = transition;
        PlaybackRuntime::start(config).unwrap()
    }

    async fn wait_for_end(events: &mut broadcast::Receiver<PlaybackEvent>) {
        timeout(Duration::from_secs(3), async {
            loop {
                if matches!(
                    events.recv().await.unwrap(),
                    PlaybackEvent::PlaybackEnded { .. }
                ) {
                    break;
                }
            }
        })
        .await
        .expect("playback should end");
    }

    #[tokio::test]
    async fn sink_disconnect_recovers_from_consumed_checkpoint() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let creates = Arc::new(AtomicUsize::new(0));
        let registry = StageRegistrySnapshot {
            decoders: vec![Arc::new(TestDecoderFactory::new())],
            transforms: Vec::new(),
            sink: Arc::new(RecoveringSinkFactory {
                id: StageId::new("test.recovering-sink").unwrap(),
                samples: Arc::clone(&samples),
                creates: Arc::clone(&creates),
            }),
        };
        let mut config = PlaybackRuntimeConfig::new(registry);
        config.block_frames = 10;
        config.pcm_ring_blocks = 2;
        config.policies.seek_fade_frames = 0;
        config.policies.recovery_backoff_ms = 0;
        let runtime = PlaybackRuntime::start(config).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();

        controller
            .switch(item(1, 40, 100), SwitchOptions::default())
            .await
            .unwrap();

        let saw_recovering = timeout(Duration::from_secs(3), async {
            let mut recovering = false;
            loop {
                match events.recv().await.unwrap() {
                    PlaybackEvent::StateChanged(PlaybackState::Recovering) => recovering = true,
                    PlaybackEvent::PlaybackEnded { .. } => break recovering,
                    _ => {},
                }
            }
        })
        .await
        .expect("playback should recover and finish");

        assert!(saw_recovering);
        assert!(creates.load(Ordering::SeqCst) >= 2);
        assert_eq!(samples.lock().unwrap().len(), 40);
        runtime.shutdown().await.unwrap();
    }

    #[test]
    fn output_gain_envelope_advances_by_audio_frames() {
        let mut envelope = OutputGainEnvelope::new(1.0);
        envelope.schedule(0.0, 4);
        let mut block = AudioBlock::new(AudioFormat {
            sample_rate: 1_000,
            channels: 2,
            channel_mask: None,
        });
        block.samples = vec![1.0; 10];
        envelope.apply(&mut block);
        assert_eq!(
            block.samples,
            vec![0.75, 0.75, 0.5, 0.5, 0.25, 0.25, 0.0, 0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn normalizer_trims_startup_delay_and_drains_exact_resampled_duration() {
        let source = AudioFormat {
            sample_rate: 44_100,
            channels: 1,
            channel_mask: None,
        };
        let target = AudioFormat {
            sample_rate: 48_000,
            channels: 2,
            channel_mask: None,
        };
        let input_frames = 1_500_usize;
        let expected_frames = ((input_frames as f64 * target.sample_rate as f64
            / source.sample_rate as f64)
            .ceil()) as usize;
        let mut normalizer = PcmNormalizer::new(source, target).unwrap();
        let mut block = AudioBlock::new(source);
        block.samples = vec![1.0; input_frames];
        normalizer.process(&mut block).unwrap();
        let mut rendered = block.samples;

        loop {
            let mut tail = AudioBlock::new(target);
            if !normalizer.drain(&mut tail).unwrap() {
                break;
            }
            rendered.extend(tail.samples);
        }

        assert_eq!(
            rendered.len(),
            expected_frames * usize::from(target.channels)
        );
        assert!(rendered.iter().any(|sample| sample.abs() > 0.5));
    }

    #[test]
    fn normalizer_preserves_a_stereo_sine_without_noise_spikes() {
        let source = AudioFormat {
            sample_rate: 44_100,
            channels: 2,
            channel_mask: None,
        };
        let target = AudioFormat {
            sample_rate: 48_000,
            channels: 2,
            channel_mask: None,
        };
        let input_frames = 44_100_usize;
        let mut input = Vec::with_capacity(input_frames * 2);
        for frame in 0..input_frames {
            let phase = std::f32::consts::TAU * 440.0 * frame as f32 / source.sample_rate as f32;
            input.extend_from_slice(&[phase.sin() * 0.5, phase.cos() * 0.25]);
        }

        let mut normalizer = PcmNormalizer::new(source, target).unwrap();
        let mut rendered = Vec::new();
        for samples in input.chunks(1024 * usize::from(source.channels)) {
            let mut block = AudioBlock::new(source);
            block.samples.extend_from_slice(samples);
            normalizer.process(&mut block).unwrap();
            rendered.extend(block.samples);
        }
        loop {
            let mut tail = AudioBlock::new(target);
            if !normalizer.drain(&mut tail).unwrap() {
                break;
            }
            rendered.extend(tail.samples);
        }

        assert!(rendered.iter().all(|sample| sample.is_finite()));
        assert!(rendered.iter().all(|sample| sample.abs() <= 0.51));
        let stable_samples = &rendered[..rendered
            .len()
            .saturating_sub(256 * usize::from(target.channels))];
        for channel in 0..usize::from(target.channels) {
            let (max_step_index, max_step) = stable_samples
                .chunks_exact(usize::from(target.channels))
                .map(|frame| frame[channel])
                .zip(
                    stable_samples
                        .chunks_exact(usize::from(target.channels))
                        .skip(1)
                        .map(|frame| frame[channel]),
                )
                .map(|(left, right)| (right - left).abs())
                .enumerate()
                .max_by(|left, right| left.1.total_cmp(&right.1))
                .unwrap();
            assert!(
                max_step < 0.08,
                "channel {channel} has a {max_step} noise spike at frame {max_step_index}"
            );
        }
    }

    #[tokio::test]
    async fn output_gain_set_before_switch_is_applied_by_sink_worker() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
        let controller = runtime.controller();
        controller
            .set_output_gain(0.5, MediaTime::ZERO)
            .await
            .unwrap();
        let mut events = controller.subscribe_events();
        controller
            .switch(item(1, 20, 100), SwitchOptions::default())
            .await
            .unwrap();
        wait_for_end(&mut events).await;
        let rendered = samples.lock().unwrap().clone();
        assert_eq!(rendered, vec![0.5; 20]);
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn preferred_sink_format_normalizes_pcm_before_opening_output() {
        let target = AudioFormat {
            sample_rate: 200,
            channels: 2,
            channel_mask: None,
        };
        let formats = Arc::new(Mutex::new(Vec::new()));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let registry = StageRegistrySnapshot {
            decoders: vec![Arc::new(TestDecoderFactory::new())],
            transforms: Vec::new(),
            sink: Arc::new(FormatAdaptingSinkFactory {
                id: StageId::new("test.format-adapting-sink").unwrap(),
                target,
                formats: Arc::clone(&formats),
                samples: Arc::clone(&samples),
            }),
        };
        let mut config = PlaybackRuntimeConfig::new(registry);
        config.block_frames = 10;
        let runtime = PlaybackRuntime::start(config).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();

        controller
            .switch(item(1, 20, 100), SwitchOptions::default())
            .await
            .unwrap();
        wait_for_end(&mut events).await;

        assert_eq!(formats.lock().unwrap().as_slice(), &[target]);
        assert_eq!(
            samples.lock().unwrap().len(),
            40 * usize::from(target.channels)
        );
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn rebuilding_output_while_idle_is_a_noop() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(TransitionPolicy::Gapless, samples);

        runtime.controller().rebuild_output().await.unwrap();

        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn controller_clones_do_not_own_runtime_and_stop_is_not_shutdown() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
        let controller = runtime.controller();
        let disposable_clone = controller.clone();
        drop(disposable_clone);

        controller
            .switch(
                item(1, 20, 100),
                SwitchOptions {
                    autoplay: false,
                    ..SwitchOptions::default()
                },
            )
            .await
            .unwrap();
        controller.stop().await.unwrap();

        let mut events = controller.subscribe_events();
        controller
            .switch(item(2, 20, 50), SwitchOptions::default())
            .await
            .unwrap();
        wait_for_end(&mut events).await;
        assert_eq!(samples.lock().unwrap().as_slice(), &[0.5; 20]);

        runtime.shutdown().await.unwrap();
        assert!(matches!(
            controller.snapshot().await,
            Err(PlaybackControlError::Closed)
        ));
    }

    #[tokio::test]
    async fn pause_and_seek_preempt_a_sink_that_keeps_returning_would_block() {
        let allow_writes = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let registry = StageRegistrySnapshot {
            decoders: vec![Arc::new(TestDecoderFactory::new())],
            transforms: Vec::new(),
            sink: Arc::new(StalledSinkFactory {
                id: StageId::new("test.stalled-sink").unwrap(),
                allow_writes: Arc::clone(&allow_writes),
            }),
        };
        let mut config = PlaybackRuntimeConfig::new(registry);
        config.block_frames = 10;
        config.pcm_ring_blocks = 2;
        let runtime = PlaybackRuntime::start(config).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(item(1, 100, 100), SwitchOptions::default())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;

        timeout(Duration::from_millis(200), controller.pause())
            .await
            .expect("pause must not wait for the stalled partial write")
            .unwrap();
        timeout(
            Duration::from_millis(200),
            controller.seek(MediaTime::from_millis(500)),
        )
        .await
        .expect("seek discard must preempt the stalled partial write")
        .unwrap();
        assert_eq!(
            controller.snapshot().await.unwrap().consumed_position,
            MediaTime::from_millis(500)
        );

        allow_writes.store(true, Ordering::SeqCst);
        controller.play().await.unwrap();
        wait_for_end(&mut events).await;
        runtime.shutdown().await.unwrap();
    }

    async fn assert_buffered_tail_is_drained(placement: TransformPlacement) {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let registry = StageRegistrySnapshot {
            decoders: vec![Arc::new(TestDecoderFactory::new())],
            transforms: vec![Arc::new(BufferingTailTransformFactory::new(placement))],
            sink: Arc::new(TestSinkFactory {
                id: StageId::new("test.sink").unwrap(),
                samples: Arc::clone(&samples),
            }),
        };
        let mut config = PlaybackRuntimeConfig::new(registry);
        config.block_frames = 10;
        let runtime = PlaybackRuntime::start(config).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(item(1, 10, 80), SwitchOptions::default())
            .await
            .unwrap();
        wait_for_end(&mut events).await;
        assert_eq!(samples.lock().unwrap().as_slice(), &[0.8; 10]);
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn buffered_pre_mix_tail_is_drained_before_end() {
        assert_buffered_tail_is_drained(TransformPlacement::PreMix).await;
    }

    #[tokio::test]
    async fn buffered_post_mix_tail_is_drained_before_end() {
        assert_buffered_tail_is_drained(TransformPlacement::PostMix).await;
    }

    #[tokio::test]
    async fn crossfade_runs_per_track_pre_mix_and_one_shared_post_mix_chain() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let pre_counts = Arc::new(Mutex::new(Vec::new()));
        let post_counts = Arc::new(Mutex::new(Vec::new()));
        let registry = StageRegistrySnapshot {
            decoders: vec![Arc::new(TestDecoderFactory::new())],
            // Register in reverse placement order to also cover deterministic planning.
            transforms: vec![
                Arc::new(CountingTransformFactory::new(
                    "test.post-mix",
                    TransformPlacement::PostMix,
                    Arc::clone(&post_counts),
                )),
                Arc::new(CountingTransformFactory::new(
                    "test.pre-mix",
                    TransformPlacement::PreMix,
                    Arc::clone(&pre_counts),
                )),
            ],
            sink: Arc::new(TestSinkFactory {
                id: StageId::new("test.sink").unwrap(),
                samples,
            }),
        };
        let mut config = PlaybackRuntimeConfig::new(registry);
        config.block_frames = 10;
        config.pcm_ring_blocks = 2;
        config.policies.transition = TransitionPolicy::Crossfade {
            duration_frames: 20,
            curve: CrossfadeCurve::EqualPower,
            fallback: CrossfadeFallback::Gapless,
        };
        let runtime = PlaybackRuntime::start(config).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(item(1, 40, 100), SwitchOptions::default())
            .await
            .unwrap();
        controller.queue_next(item(2, 40, 50)).await.unwrap();
        wait_for_end(&mut events).await;

        let pre = pre_counts.lock().unwrap().clone();
        let post = post_counts.lock().unwrap().clone();
        assert_eq!(pre.len(), 2);
        assert!(pre.iter().all(|count| *count > 0));
        assert_eq!(post.len(), 2);
        assert!(post[0] > 0, "the shared current post-mix chain must run");
        assert_eq!(
            post[1], 0,
            "the next per-track path must not run post-mix DSP"
        );
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn crossfade_normalizes_next_sample_rate_and_channels_before_mixing() {
        let formats = Arc::new(Mutex::new(Vec::new()));
        let creates = Arc::new(AtomicUsize::new(0));
        let sink = Arc::new(RecordingSinkFactory {
            id: StageId::new("test.recording-sink").unwrap(),
            formats: Arc::clone(&formats),
            creates: Arc::clone(&creates),
        });
        let registry = StageRegistrySnapshot {
            decoders: Vec::new(),
            transforms: Vec::new(),
            sink,
        };
        let mut config = PlaybackRuntimeConfig::new(registry);
        config.block_frames = 64;
        config.policies.transition = TransitionPolicy::Crossfade {
            duration_frames: 20,
            curve: CrossfadeCurve::Linear,
            fallback: CrossfadeFallback::Gapless,
        };
        let runtime = PlaybackRuntime::start(config).unwrap();
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        let current_format = AudioFormat {
            sample_rate: 100,
            channels: 1,
            channel_mask: None,
        };
        let next_format = AudioFormat {
            sample_rate: 200,
            channels: 2,
            channel_mask: None,
        };
        controller
            .switch(
                fixed_format_item(
                    1,
                    Arc::new(FixedFormatDecoderFactory::new(
                        "test.current-format",
                        current_format,
                        80,
                        1.0,
                    )),
                ),
                SwitchOptions::default(),
            )
            .await
            .unwrap();
        controller
            .queue_next(fixed_format_item(
                2,
                Arc::new(FixedFormatDecoderFactory::new(
                    "test.next-format",
                    next_format,
                    160,
                    0.5,
                )),
            ))
            .await
            .unwrap();
        wait_for_end(&mut events).await;

        assert_eq!(creates.load(Ordering::SeqCst), 1, "output must be reused");
        let written_formats = formats.lock().unwrap().clone();
        assert!(!written_formats.is_empty());
        assert!(
            written_formats
                .iter()
                .all(|format| *format == current_format)
        );
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn gapless_reuses_sink_and_reports_consumed_boundary() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(
                item(1, 40, 100),
                SwitchOptions {
                    autoplay: false,
                    ..SwitchOptions::default()
                },
            )
            .await
            .unwrap();
        controller.queue_next(item(2, 40, 50)).await.unwrap();
        controller.play().await.unwrap();
        wait_for_end(&mut events).await;
        let output = samples.lock().unwrap().clone();
        assert_eq!(output.len(), 80);
        assert_eq!(&output[..40], vec![1.0; 40]);
        assert_eq!(&output[40..], vec![0.5; 40]);
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn fade_out_in_is_sequential_and_never_overlaps_track_pipelines() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(
            TransitionPolicy::FadeOutIn {
                fade_out_frames: 10,
                fade_in_frames: 10,
                curve: GainCurve::Linear,
            },
            Arc::clone(&samples),
        );
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(
                item(1, 40, 100),
                SwitchOptions {
                    autoplay: false,
                    ..SwitchOptions::default()
                },
            )
            .await
            .unwrap();
        controller.queue_next(item(2, 40, 100)).await.unwrap();
        controller.play().await.unwrap();
        wait_for_end(&mut events).await;

        let output = samples.lock().unwrap().clone();
        assert_eq!(output.len(), 80, "sequential fade must not overlap PCM");
        assert!(output[30] > output[39], "current track must fade out");
        assert!(output[40] < output[49], "next track must fade in");
        assert_eq!(output[40], 0.0);
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn eof_waits_in_buffering_for_slow_next_preparation_then_promotes() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(TransitionPolicy::Gapless, Arc::clone(&samples));
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(item(1, 30, 100), SwitchOptions::default())
            .await
            .unwrap();
        let queued = {
            let controller = controller.clone();
            tokio::spawn(async move {
                controller
                    .queue_next(delayed_item(2, 20, 50, Duration::from_millis(80)))
                    .await
            })
        };

        let (saw_waiting, saw_next) = timeout(Duration::from_secs(3), async {
            let mut waiting = false;
            let mut next = false;
            loop {
                match events.recv().await.unwrap() {
                    PlaybackEvent::Buffering {
                        item_id,
                        active: true,
                    } if item_id == PlaybackItemId::new(1).unwrap() => waiting = true,
                    PlaybackEvent::TrackChanged { item_id }
                        if item_id == PlaybackItemId::new(2).unwrap() =>
                    {
                        next = true;
                    },
                    PlaybackEvent::PlaybackEnded { item_id }
                        if item_id == PlaybackItemId::new(2).unwrap() =>
                    {
                        break (waiting, next);
                    },
                    _ => {},
                }
            }
        })
        .await
        .expect("slow next should eventually promote");
        queued.await.unwrap().unwrap();
        assert!(saw_waiting);
        assert!(saw_next);
        assert_eq!(samples.lock().unwrap().len(), 50);
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn newer_switch_cancels_a_stale_slow_source_open() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(TransitionPolicy::Gapless, samples);
        let controller = runtime.controller();
        let first = {
            let controller = controller.clone();
            tokio::spawn(async move {
                controller
                    .switch(
                        delayed_item(1, 20, 100, Duration::from_millis(500)),
                        SwitchOptions::default(),
                    )
                    .await
            })
        };
        tokio::time::sleep(Duration::from_millis(20)).await;
        timeout(
            Duration::from_millis(200),
            controller.switch(item(2, 20, 50), SwitchOptions::default()),
        )
        .await
        .expect("new switch must not wait for the stale source open")
        .unwrap();
        assert!(matches!(
            first.await.unwrap(),
            Err(PlaybackControlError::Closed)
        ));
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn crossfade_overlaps_two_track_pipelines() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(
            TransitionPolicy::Crossfade {
                duration_frames: 20,
                curve: CrossfadeCurve::Linear,
                fallback: CrossfadeFallback::Gapless,
            },
            Arc::clone(&samples),
        );
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(
                item(1, 100, 100),
                SwitchOptions {
                    autoplay: false,
                    ..SwitchOptions::default()
                },
            )
            .await
            .unwrap();
        controller.queue_next(item(2, 100, 0)).await.unwrap();
        controller.play().await.unwrap();
        wait_for_end(&mut events).await;
        let output = samples.lock().unwrap().clone();
        assert_eq!(
            output.len(),
            180,
            "20 frames must overlap instead of concatenate"
        );
        assert!(output[80] > output[99]);
        assert!(output[99] < 0.1);
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn next_failure_during_crossfade_is_typed_and_current_loses_no_frames() {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let runtime = runtime(
            TransitionPolicy::Crossfade {
                duration_frames: 20,
                curve: CrossfadeCurve::Linear,
                fallback: CrossfadeFallback::Gapless,
            },
            Arc::clone(&samples),
        );
        let controller = runtime.controller();
        let mut events = controller.subscribe_events();
        controller
            .switch(item(1, 100, 100), SwitchOptions::default())
            .await
            .unwrap();
        controller
            .queue_next(fixed_format_item(
                2,
                Arc::new(FailingNextDecoderFactory::new()),
            ))
            .await
            .unwrap();

        let failure = timeout(Duration::from_secs(3), async {
            let mut failure = None;
            loop {
                match events.recv().await.unwrap() {
                    PlaybackEvent::Failed(value) => failure = Some(value),
                    PlaybackEvent::PlaybackEnded { .. } => break failure.unwrap(),
                    _ => {},
                }
            }
        })
        .await
        .expect("current track should recover from next failure and end");
        assert_eq!(failure.item_id, PlaybackItemId::new(2));
        assert_eq!(failure.stage, stellatune_audio_core::FailureStage::Decoder);
        assert!(failure.generation > 0);
        assert_eq!(samples.lock().unwrap().len(), 100);
        runtime.shutdown().await.unwrap();
    }
}
