use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TrySendError};
use stellatune_audio_core::{
    error::PlaybackControlError,
    format::{AudioBlock, PcmFormat},
    playback::PlaybackItemId,
    sink::{SinkClockSnapshot, SinkFactory, SinkStage, SinkWriteState},
};
pub(super) enum PendingWrite {
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

pub(super) struct SinkWorker {
    data_sender: Sender<SinkDataCommand>,
    control_sender: Sender<SinkControlCommand>,
    boundary_receiver: Receiver<PlaybackItemId>,
    failure_receiver: Receiver<String>,
    clock: Arc<SinkWorkerClock>,
    join: Option<JoinHandle<()>>,
}

impl SinkWorker {
    pub(super) fn start(
        factory: Arc<dyn SinkFactory>,
        format: PcmFormat,
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

    pub(super) fn pause(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Pause)
    }

    pub(super) fn resume(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Resume)
    }

    pub(super) fn discard(&self, epoch: u64) -> Result<(), PlaybackControlError> {
        self.control(|response| SinkControlCommand::Discard { epoch, response })
    }

    pub(super) fn set_gain(
        &self,
        target: f32,
        duration_frames: u64,
    ) -> Result<(), PlaybackControlError> {
        self.control_sender
            .send(SinkControlCommand::SetGain {
                target,
                duration_frames,
            })
            .map_err(|_| PlaybackControlError::Closed)
    }

    pub(super) fn drain(&self) -> Result<(), PlaybackControlError> {
        self.control(SinkControlCommand::Drain)
    }

    pub(super) fn mark_boundary(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<(), PlaybackControlError> {
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

    pub(super) fn try_boundary(&self) -> Option<PlaybackItemId> {
        self.boundary_receiver.try_recv().ok()
    }

    pub(super) fn try_failure(&self) -> Option<String> {
        self.failure_receiver.try_recv().ok()
    }

    pub(super) fn clock(&self) -> SinkClockSnapshot {
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

    pub(super) fn shutdown(&mut self) {
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

pub(super) struct OutputGainEnvelope {
    start: f32,
    current: f32,
    target: f32,
    progressed_frames: u64,
    duration_frames: u64,
}

impl OutputGainEnvelope {
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

    pub(super) fn schedule(&mut self, target: f32, duration_frames: u64) {
        self.start = self.current;
        self.target = target.clamp(0.0, 1.0);
        self.progressed_frames = 0;
        self.duration_frames = duration_frames;
        if duration_frames == 0 {
            self.current = self.target;
        }
    }

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
