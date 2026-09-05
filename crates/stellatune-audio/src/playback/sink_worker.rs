//! Dedicated output thread, bounded PCM transport, and consumed-frame clock.
//!
//! `SinkWorker` isolates device calls from the playback actor. PCM blocks and
//! item-boundary markers share one bounded FIFO data channel, preserving their
//! order. Pause, resume, discard, gain, and drain use a separate bounded control
//! channel consumed with priority over PCM. Shutdown uses an independent atomic
//! flag, so a full control queue cannot prevent teardown.
//!
//! Opening, control acknowledgements, and drain completion are polled by the actor.
//! No actor-side operation waits for a device call. Retired threads are joined by
//! runtime shutdown on the blocking executor. Device ownership is serialized across
//! workers so a replacement opens only after the previous endpoint closes.
//!
//! Partial writes remain on the worker thread until accepted. The actor-facing
//! clock combines PCM queued in the data channel with the sink's own device
//! buffer. A discard installs a new epoch and drops every block and boundary
//! from the previous epoch. A track boundary becomes observable only after the
//! device-consumed clock reaches its accepted-frame position.

use super::output_workers::OutputWorkers;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use stellatune_audio_core::{error::FailureStage, stage::StageId};
type ControlResult = Result<(), PlaybackControlError>;
use lattice_actor::reply::ReplyTo;

struct PendingControl {
    response: std::sync::mpsc::Receiver<ControlResult>,
    replies: Vec<ReplyTo<ControlResult>>,
}

impl Drop for PendingControl {
    fn drop(&mut self) {
        for reply in self.replies.drain(..) {
            let _ = reply.send(Err(PlaybackControlError::Closed));
        }
    }
}
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TrySendError};
use stellatune_audio_core::{
    error::PlaybackControlError,
    format::{AudioBlock, PcmFormat},
    playback::PlaybackItemId,
    sink::{SinkClockSnapshot, SinkFactory, SinkStage, SinkWriteState},
};
/// A non-blocking actor-to-worker enqueue failure.
pub(super) enum PendingWrite {
    /// The ring is full and ownership of the block is returned for retry.
    Full(AudioBlock),
    /// The sink worker has stopped and cannot accept the block.
    Closed,
}

enum SinkDataCommand {
    Write(AudioBlock),
    Boundary { item_id: PlaybackItemId, epoch: u64 },
}

enum SinkControlCommand {
    Pause(std::sync::mpsc::Sender<ControlResult>),
    Resume(std::sync::mpsc::Sender<ControlResult>),
    Discard {
        epoch: u64,
        response: std::sync::mpsc::Sender<ControlResult>,
    },
    SetGain {
        target: f32,
        duration_frames: u64,
    },
    Drain(std::sync::mpsc::Sender<ControlResult>),
}

struct SinkWorkerClock {
    consumed_frames: AtomicU64,
    buffered_frames: AtomicU64,
    device_buffered_frames: AtomicU64,
    epoch: AtomicU64,
}

/// Actor-facing handle to one sink and its dedicated OS thread.
pub(super) struct SinkWorker {
    data_sender: Sender<SinkDataCommand>,
    control_sender: Sender<SinkControlCommand>,
    boundary_receiver: Receiver<PlaybackItemId>,
    failure_receiver: Receiver<PlaybackControlError>,
    pending_controls: std::cell::RefCell<Vec<PendingControl>>,
    pending_drain: Option<std::sync::mpsc::Receiver<ControlResult>>,
    stopped: Arc<AtomicBool>,
    initialized: Arc<AtomicBool>,
    clock: Arc<SinkWorkerClock>,
    epoch: u64,
}

impl SinkWorker {
    /// Starts asynchronous opening on a new paused output thread.
    pub(super) fn start(
        factory: Arc<dyn SinkFactory>,
        format: PcmFormat,
        capacity: usize,
        initial_gain: f32,
        workers: &OutputWorkers,
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
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        let initialized = Arc::new(AtomicBool::new(false));
        let worker_initialized = Arc::clone(&initialized);
        let device = Arc::clone(&workers.device);
        let stage_id = factory.id().clone();
        let join = std::thread::Builder::new()
            .name("stellatune-sink-worker".to_owned())
            .spawn(move || {
                let _device = device.lock().unwrap_or_else(|error| error.into_inner());
                if worker_stopped.load(Ordering::Acquire) {
                    return;
                }
                let mut sink = match factory.create() {
                    Ok(sink) => sink,
                    Err(error) => {
                        let _ = started_tx.send(Err(PlaybackControlError::factory(
                            FailureStage::Sink,
                            stage_id.clone(),
                            error,
                        )));
                        return;
                    },
                };
                if let Err(error) = sink.open(format) {
                    let _ =
                        started_tx.send(Err(PlaybackControlError::sink(error, stage_id.clone())));
                    sink.close();
                    return;
                }
                if let Err(error) = sink.pause() {
                    let _ =
                        started_tx.send(Err(PlaybackControlError::sink(error, stage_id.clone())));
                    sink.close();
                    return;
                }
                worker_initialized.store(true, Ordering::Release);
                let _ = started_tx.send(Ok(()));
                sink_worker_loop(
                    sink,
                    data_receiver,
                    control_receiver,
                    worker_clock,
                    boundary_sender,
                    failure_sender,
                    initial_gain,
                    worker_stopped,
                    stage_id,
                );
            })
            .map_err(|error| PlaybackControlError::failed(FailureStage::Sink, error.to_string()))?;
        workers.register(join);
        Ok(Self {
            data_sender,
            control_sender,
            boundary_receiver,
            failure_receiver,
            clock,
            stopped,
            initialized,
            pending_controls: std::cell::RefCell::new(vec![PendingControl {
                response: started_rx,
                replies: Vec::new(),
            }]),
            pending_drain: None,
            epoch: 0,
        })
    }

    /// Attempts to enqueue a PCM block without waiting for ring capacity.
    pub(super) fn try_write(&self, block: AudioBlock) -> Result<(), PendingWrite> {
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

    fn enqueue(&self, command: SinkControlCommand) -> ControlResult {
        self.control_sender
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Disconnected(_) => PlaybackControlError::Closed,
                TrySendError::Full(_) => {
                    PlaybackControlError::failed(FailureStage::Sink, "output control queue is full")
                },
            })
    }

    fn control(
        &self,
        build: impl FnOnce(std::sync::mpsc::Sender<ControlResult>) -> SinkControlCommand,
    ) -> ControlResult {
        let (tx, rx) = std::sync::mpsc::channel();
        self.enqueue(build(tx))?;
        self.pending_controls.borrow_mut().push(PendingControl {
            response: rx,
            replies: Vec::new(),
        });
        Ok(())
    }

    /// Associates an external command reply with actual device acknowledgement.
    pub(super) fn request_playing(
        &self,
        playing: bool,
        reply: ReplyTo<ControlResult>,
    ) -> ControlResult {
        let (tx, rx) = std::sync::mpsc::channel();
        let command = if playing {
            SinkControlCommand::Resume(tx)
        } else {
            SinkControlCommand::Pause(tx)
        };
        if let Err(error) = self.enqueue(command) {
            let _ = reply.send(Err(error.clone()));
            return Err(error);
        }
        self.pending_controls.borrow_mut().push(PendingControl {
            response: rx,
            replies: vec![reply],
        });
        Ok(())
    }

    /// Defers initial activation acknowledgement until sink open has completed.
    pub(super) fn reply_when_started(&self, reply: ReplyTo<ControlResult>) {
        if let Some(startup) = self.pending_controls.borrow_mut().first_mut() {
            startup.replies.push(reply);
        } else {
            let _ = reply.send(if self.is_initialized() {
                Ok(())
            } else {
                Err(PlaybackControlError::Closed)
            });
        }
    }

    /// Waits for the latest enqueued control without holding the actor turn.
    pub(super) fn reply_when_settled(&self, reply: ReplyTo<ControlResult>) {
        if let Some(pending) = self.pending_controls.borrow_mut().last_mut() {
            pending.replies.push(reply);
        } else {
            let _ = reply.send(Ok(()));
        }
    }

    /// Pauses the sink without waiting for the device; failures are polled by the actor.
    pub(super) fn pause(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Pause)
    }

    /// Resumes the sink without waiting for the device; failures are polled by the actor.
    pub(super) fn resume(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Resume)
    }

    pub(super) fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Advances the output-owned epoch and asynchronously invalidates old PCM.
    pub(super) fn discard(&mut self) -> Result<(), PlaybackControlError> {
        let epoch = self.epoch.wrapping_add(1);
        self.control(|response| SinkControlCommand::Discard { epoch, response })?;
        self.pending_drain = None;
        self.epoch = epoch;
        Ok(())
    }

    /// Schedules a final linear gain ramp without waiting for its completion.
    pub(super) fn set_gain(
        &self,
        target: f32,
        duration_frames: u64,
    ) -> Result<(), PlaybackControlError> {
        self.enqueue(SinkControlCommand::SetGain {
            target,
            duration_frames,
        })
    }

    /// Polls a drain request, retaining the output until its final PCM is consumed.
    pub(super) fn drain(&mut self) -> Result<bool, PlaybackControlError> {
        if self.pending_drain.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            self.enqueue(SinkControlCommand::Drain(tx))?;
            self.pending_drain = Some(rx);
        }
        match self.pending_drain.as_ref().unwrap().try_recv() {
            Ok(result) => {
                self.pending_drain = None;
                result.map(|()| true)
            },
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(false),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(PlaybackControlError::Closed),
        }
    }

    /// Queues an item marker after all PCM accepted before this call.
    /// Returns false when the FIFO is full; the caller must retain and retry it.
    pub(super) fn mark_boundary(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<bool, PlaybackControlError> {
        match self.data_sender.try_send(SinkDataCommand::Boundary {
            item_id,
            epoch: self.epoch,
        }) {
            Ok(()) => Ok(true),
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => Err(PlaybackControlError::Closed),
        }
    }

    /// Returns the next item boundary consumed by the device, if any.
    pub(super) fn try_boundary(&self) -> Option<PlaybackItemId> {
        self.boundary_receiver.try_recv().ok()
    }

    pub(super) fn is_ready(&self) -> bool {
        self.is_initialized() && self.clock.epoch.load(Ordering::Acquire) == self.epoch
    }

    pub(super) fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }

    /// Returns the first pending sink-worker failure, if any.
    pub(super) fn try_failure(&self) -> Option<PlaybackControlError> {
        let mut failure = self.failure_receiver.try_recv().ok();
        self.pending_controls.borrow_mut().retain_mut(|pending| {
            let result = match pending.response.try_recv() {
                Ok(result) => result,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Err(PlaybackControlError::Closed)
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => return true,
            };
            if let Err(error) = &result {
                failure.get_or_insert(error.clone());
            }
            for reply in pending.replies.drain(..) {
                let _ = reply.send(result.clone());
            }
            false
        });
        failure
    }

    /// Returns the latest device clock plus actor-to-worker queued frames.
    pub(super) fn clock(&self) -> SinkClockSnapshot {
        if self.clock.epoch.load(Ordering::Acquire) != self.epoch {
            return SinkClockSnapshot {
                consumed_frames: 0,
                buffered_frames: 0,
                epoch: self.epoch,
            };
        }
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

    /// Requests shutdown independently of control queue capacity; never joins here.
    pub(super) fn shutdown(&mut self) {
        self.stopped.store(true, Ordering::Release);
    }
}

impl Drop for SinkWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// A frame-counted final gain envelope owned by the sink worker.
pub(super) struct OutputGainEnvelope {
    start: f32,
    current: f32,
    target: f32,
    progressed_frames: u64,
    duration_frames: u64,
}

impl OutputGainEnvelope {
    /// Creates a constant envelope with a clamped initial gain.
    pub(super) fn new(initial: f32) -> Self {
        let initial = initial.clamp(0.0, 1.0);
        Self {
            start: initial,
            current: initial,
            target: initial,
            progressed_frames: 0,
            duration_frames: 0,
        }
    }

    /// Replaces the active ramp, starting at its instantaneous gain.
    pub(super) fn schedule(&mut self, target: f32, duration_frames: u64) {
        self.start = self.current;
        self.target = target.clamp(0.0, 1.0);
        self.progressed_frames = 0;
        self.duration_frames = duration_frames;
        if duration_frames == 0 {
            self.current = self.target;
        }
    }

    /// Applies the scheduled gain independently to every complete PCM frame.
    pub(super) fn apply(&mut self, block: &mut AudioBlock) {
        let channels = usize::from(block.format.channel_layout.channel_count());
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

/// Services prioritized control and partial PCM writes until shutdown or failure.
#[allow(clippy::too_many_arguments)]
fn sink_worker_loop(
    mut sink: Box<dyn SinkStage>,
    data_receiver: Receiver<SinkDataCommand>,
    control_receiver: Receiver<SinkControlCommand>,
    clock: Arc<SinkWorkerClock>,
    boundary_sender: Sender<PlaybackItemId>,
    failure_sender: Sender<PlaybackControlError>,
    initial_gain: f32,
    stopped: Arc<AtomicBool>,
    stage_id: StageId,
) {
    let mut gain = OutputGainEnvelope::new(initial_gain);
    let mut accepted_frames = 0_u64;
    let mut pending_boundaries = VecDeque::<(u64, PlaybackItemId)>::new();
    let mut pending_block = None::<AudioBlock>;
    let mut pending_drain = None::<std::sync::mpsc::Sender<ControlResult>>;
    let mut paused = true;
    'worker: loop {
        if stopped.load(Ordering::Acquire) {
            break;
        }
        while let Ok(command) = control_receiver.try_recv() {
            if handle_sink_control(
                command,
                sink.as_mut(),
                clock.as_ref(),
                &mut gain,
                &mut paused,
                &mut pending_block,
                &mut pending_drain,
                &mut accepted_frames,
                &mut pending_boundaries,
                &stage_id,
            ) {
                break 'worker;
            }
        }

        if pending_drain.is_some()
            && pending_block.is_none()
            && data_receiver.is_empty()
            && let Some(response) = pending_drain.take()
        {
            let result = sink
                .drain()
                .map_err(|error| PlaybackControlError::sink(error, stage_id.clone()));
            // Publish the drained device position before the actor can observe
            // completion and emit the final position or release this output.
            sync_sink_clock(sink.as_ref(), clock.as_ref());
            let _ = response.send(result);
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
                            let samples = consumed.saturating_mul(usize::from(
                                block.format.channel_layout.channel_count(),
                            ));
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
                        let _ = failure_sender
                            .try_send(PlaybackControlError::sink(error, stage_id.clone()));
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
                                                clock.as_ref(),
                                &mut gain,
                                &mut paused,
                                &mut pending_block,
                                &mut pending_drain,
                                &mut accepted_frames,
                                &mut pending_boundaries,
                                &stage_id,
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
                        clock.as_ref(),
                        &mut gain,
                        &mut paused,
                        &mut pending_block,
                        &mut pending_drain,
                        &mut accepted_frames,
                        &mut pending_boundaries,
                        &stage_id,
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

/// Accepts one ordered PCM block or records its device-consumption boundary.
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
        SinkDataCommand::Boundary { item_id, epoch } => {
            if epoch != clock.epoch.load(Ordering::Acquire) {
                return;
            }
            pending_boundaries.push_back((accepted_frames, item_id));
        },
    }
}

/// Applies one high-priority control command and reports whether to stop.
#[allow(clippy::too_many_arguments)]
fn handle_sink_control(
    command: SinkControlCommand,
    sink: &mut dyn SinkStage,
    clock: &SinkWorkerClock,
    gain: &mut OutputGainEnvelope,
    paused: &mut bool,
    pending_block: &mut Option<AudioBlock>,
    pending_drain: &mut Option<std::sync::mpsc::Sender<ControlResult>>,
    accepted_frames: &mut u64,
    pending_boundaries: &mut VecDeque<(u64, PlaybackItemId)>,
    stage_id: &StageId,
) -> bool {
    match command {
        SinkControlCommand::Pause(response) => {
            let result = sink
                .pause()
                .map_err(|error| PlaybackControlError::sink(error, stage_id.clone()));
            if result.is_ok() {
                *paused = true;
            }
            let _ = response.send(result);
        },
        SinkControlCommand::Resume(response) => {
            let result = sink
                .resume()
                .map_err(|error| PlaybackControlError::sink(error, stage_id.clone()));
            if result.is_ok() {
                *paused = false;
            }
            let _ = response.send(result);
        },
        SinkControlCommand::Discard { epoch, response } => {
            let result = sink
                .discard()
                .map_err(|error| PlaybackControlError::sink(error, stage_id.clone()));
            if let Some(block) = pending_block.take() {
                clock
                    .buffered_frames
                    .fetch_sub(block.frames() as u64, Ordering::Relaxed);
            }
            if let Some(drain) = pending_drain.take() {
                let _ = drain.send(Err(PlaybackControlError::Closed));
            }
            clock.consumed_frames.store(0, Ordering::Relaxed);
            clock.device_buffered_frames.store(0, Ordering::Relaxed);
            clock.epoch.store(epoch, Ordering::Release);
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
                let _ = replaced.send(Err(PlaybackControlError::Closed));
            }
        },
    }
    false
}

/// Copies the sink clock into atomics visible to the playback actor.
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

/// Publishes every ordered marker reached by the device-consumed frontier.
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
