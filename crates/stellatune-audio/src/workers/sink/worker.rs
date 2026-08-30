//! Dedicated sink worker loop and control channel plumbing.
//!
//! This module decouples decode-thread production from sink I/O timing by using
//! a bounded ring buffer and a control mailbox.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TrySendError};
use ringbuf::traits::{Consumer as _, Producer as _, Split as _};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageFlow;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;

enum SinkControl {
    RefreshRuntimeState {
        ctx: PipelineContext,
        resp_tx: Sender<Result<(), PipelineError>>,
    },
    Drain {
        resp_tx: Sender<Result<(), PipelineError>>,
    },
    DropQueued {
        resp_tx: Sender<Result<(), PipelineError>>,
    },
    Shutdown {
        drain: bool,
        resp_tx: Sender<Result<(), PipelineError>>,
    },
}

#[derive(Debug)]
pub(crate) enum SinkWriteError {
    Full(AudioBlock),
    Disconnected,
}

pub(crate) struct SinkWorker {
    audio_prod: HeapProd<AudioBlock>,
    wake_tx: Sender<()>,
    ctrl_tx: Sender<SinkControl>,
    running: Arc<AtomicBool>,
    consumed_frames: Arc<AtomicU64>,
    position_origin_ms: Arc<AtomicI64>,
    sample_rate: u32,
    join: Option<JoinHandle<()>>,
}

impl SinkWorker {
    pub(crate) fn start(
        sinks: Vec<Box<dyn SinkStage>>,
        spec: StreamSpec,
        initial_ctx: PipelineContext,
        queue_capacity: usize,
    ) -> Result<Self, PipelineError> {
        let capacity = queue_capacity.max(1);
        let rb = HeapRb::<AudioBlock>::new(capacity);
        let (audio_prod, audio_cons) = rb.split();
        let (wake_tx, wake_rx) = crossbeam_channel::bounded::<()>(1);
        let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded::<SinkControl>();
        let (startup_tx, startup_rx) = crossbeam_channel::bounded::<Result<(), PipelineError>>(1);
        let running = Arc::new(AtomicBool::new(true));
        let running_for_thread = Arc::clone(&running);
        let consumed_frames = Arc::new(AtomicU64::new(0));
        let consumed_frames_for_thread = Arc::clone(&consumed_frames);
        let position_origin_ms = Arc::new(AtomicI64::new(initial_ctx.position_ms.max(0)));

        let join = std::thread::Builder::new()
            .name("stellatune-audio-sink-loop".to_string())
            .spawn(move || {
                let _rt_guard = crate::infra::realtime::enable_realtime_audio_thread();
                sink_thread_main(SinkThreadArgs {
                    sinks,
                    spec,
                    ctx: initial_ctx,
                    startup_tx,
                    audio_cons,
                    wake_rx,
                    ctrl_rx,
                    running: running_for_thread,
                    consumed_frames: consumed_frames_for_thread,
                })
            })
            .map_err(|e| PipelineError::StageFailure(format!("spawn sink loop failed: {e}")))?;

        let startup = startup_rx.recv().map_err(|_| {
            PipelineError::StageFailure("sink loop startup channel closed".to_string())
        })?;
        if let Err(error) = startup {
            let _ = join.join();
            return Err(error);
        }

        Ok(Self {
            audio_prod,
            wake_tx,
            ctrl_tx,
            running,
            consumed_frames,
            position_origin_ms,
            sample_rate: spec.sample_rate.max(1),
            join: Some(join),
        })
    }

    pub(crate) fn try_send_block(&mut self, block: AudioBlock) -> Result<(), SinkWriteError> {
        if !self.running.load(Ordering::Acquire) {
            return Err(SinkWriteError::Disconnected);
        }

        match self.audio_prod.try_push(block) {
            Ok(()) => match self.wake_tx.try_send(()) {
                Ok(()) | Err(TrySendError::Full(())) => Ok(()),
                Err(TrySendError::Disconnected(_)) => Err(SinkWriteError::Disconnected),
            },
            Err(block) => {
                if self.running.load(Ordering::Acquire) {
                    Err(SinkWriteError::Full(block))
                } else {
                    Err(SinkWriteError::Disconnected)
                }
            },
        }
    }

    pub(crate) fn refresh_runtime_state(
        &self,
        ctx: &PipelineContext,
        timeout: Duration,
    ) -> Result<(), PipelineError> {
        self.call_control(
            |resp_tx| SinkControl::RefreshRuntimeState {
                ctx: ctx.clone(),
                resp_tx,
            },
            timeout,
        )
    }

    pub(crate) fn drain(&self, timeout: Duration) -> Result<(), PipelineError> {
        self.call_control(|resp_tx| SinkControl::Drain { resp_tx }, timeout)
    }

    pub(crate) fn drop_queued(&self, timeout: Duration) -> Result<(), PipelineError> {
        self.call_control(|resp_tx| SinkControl::DropQueued { resp_tx }, timeout)
    }

    pub(crate) fn reset_consumed_position(&self, position_ms: i64) {
        self.position_origin_ms
            .store(position_ms.max(0), Ordering::Release);
        self.consumed_frames.store(0, Ordering::Release);
    }

    pub(crate) fn consumed_position_ms(&self) -> i64 {
        let frames = self.consumed_frames.load(Ordering::Acquire) as u128;
        let elapsed_ms = frames.saturating_mul(1_000) / self.sample_rate as u128;
        self.position_origin_ms
            .load(Ordering::Acquire)
            .saturating_add(elapsed_ms.min(i64::MAX as u128) as i64)
    }

    pub(crate) fn shutdown(mut self, drain: bool, timeout: Duration) -> Result<(), PipelineError> {
        let shutdown_result =
            self.call_control(|resp_tx| SinkControl::Shutdown { drain, resp_tx }, timeout);
        if let Some(join) = self.join.take() {
            detach_join(join);
        }
        self.running.store(false, Ordering::Release);
        shutdown_result
    }

    /// Sends a control request to sink loop and maps timeout/disconnect to pipeline errors.
    fn call_control(
        &self,
        constructor: impl FnOnce(Sender<Result<(), PipelineError>>) -> SinkControl,
        timeout: Duration,
    ) -> Result<(), PipelineError> {
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        self.ctrl_tx
            .send(constructor(resp_tx))
            .map_err(|_| PipelineError::SinkDisconnected)?;
        resp_rx.recv_timeout(timeout).map_err(|error| match error {
            RecvTimeoutError::Timeout => PipelineError::StageFailure(format!(
                "sink loop control timed out after {}ms",
                timeout.as_millis()
            )),
            RecvTimeoutError::Disconnected => PipelineError::SinkDisconnected,
        })?
    }
}

impl Drop for SinkWorker {
    fn drop(&mut self) {
        if self.join.is_none() {
            return;
        }

        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        let _ = self.ctrl_tx.send(SinkControl::Shutdown {
            drain: false,
            resp_tx,
        });
        let _ = resp_rx.recv_timeout(Duration::from_millis(100));
        if let Some(join) = self.join.take() {
            detach_join(join);
        }
        self.running.store(false, Ordering::Release);
    }
}

fn detach_join(join: JoinHandle<()>) {
    let _ = std::thread::Builder::new()
        .name("stellatune-audio-sink-join".to_string())
        .spawn(move || {
            let _ = join.join();
        });
}

struct RunningFlagGuard {
    running: Arc<AtomicBool>,
}

impl RunningFlagGuard {
    fn new(running: Arc<AtomicBool>) -> Self {
        Self { running }
    }
}

impl Drop for RunningFlagGuard {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

struct SinkThreadArgs {
    sinks: Vec<Box<dyn SinkStage>>,
    spec: StreamSpec,
    ctx: PipelineContext,
    startup_tx: Sender<Result<(), PipelineError>>,
    audio_cons: HeapCons<AudioBlock>,
    wake_rx: Receiver<()>,
    ctrl_rx: Receiver<SinkControl>,
    running: Arc<AtomicBool>,
    consumed_frames: Arc<AtomicU64>,
}

struct ControlOutcome {
    break_loop: bool,
    sinks_stopped: bool,
}

/// Entry point for the sink thread.
///
/// The loop waits for either control commands or wake events and processes
/// queued audio blocks until shutdown or channel disconnect.
fn sink_thread_main(args: SinkThreadArgs) {
    let SinkThreadArgs {
        mut sinks,
        spec,
        mut ctx,
        startup_tx,
        mut audio_cons,
        wake_rx,
        ctrl_rx,
        running,
        consumed_frames,
    } = args;
    let _running_guard = RunningFlagGuard::new(running);
    let startup = prepare_sinks(&mut sinks, spec, &mut ctx);
    let startup_ok = startup.is_ok();
    if startup_tx.send(startup).is_err() {
        return;
    }
    if !startup_ok {
        return;
    }

    let mut sinks_stopped = false;

    loop {
        crossbeam_channel::select! {
            recv(ctrl_rx) -> msg => {
                let Ok(control) = msg else {
                    break;
                };
                // Control commands are handled eagerly to keep external RPCs responsive.
                let outcome = handle_control(
                    control,
                    &mut sinks,
                    &mut audio_cons,
                    &mut ctx,
                    &consumed_frames,
                );
                sinks_stopped |= outcome.sinks_stopped;
                if outcome.break_loop {
                    break;
                }
            }
            recv(wake_rx) -> msg => {
                if msg.is_err() {
                    break;
                }
                if drain_audio_ring_to_sinks(
                    &mut sinks,
                    &mut audio_cons,
                    &mut ctx,
                    &consumed_frames,
                ).is_err() {
                    break;
                }
            }
        }
    }

    if !sinks_stopped {
        stop_sinks(&mut sinks, &mut ctx);
    }
}

fn prepare_sinks(
    sinks: &mut [Box<dyn SinkStage>],
    spec: StreamSpec,
    ctx: &mut PipelineContext,
) -> Result<(), PipelineError> {
    for sink in sinks {
        sink.prepare(spec, ctx)?;
    }
    Ok(())
}

fn write_block(
    sinks: &mut [Box<dyn SinkStage>],
    block: &AudioBlock,
    ctx: &mut PipelineContext,
) -> Result<(), PipelineError> {
    for sink in sinks {
        match sink.write(block, ctx)? {
            StageFlow::Continue => {},
            StageFlow::Eof => {
                return Err(PipelineError::StageFailure("sink reached eof".to_string()));
            },
        }
    }
    Ok(())
}

fn flush_sinks(
    sinks: &mut [Box<dyn SinkStage>],
    ctx: &mut PipelineContext,
) -> Result<(), PipelineError> {
    for sink in sinks {
        sink.flush(ctx)?;
    }
    Ok(())
}

fn drain_audio_ring_to_sinks(
    sinks: &mut [Box<dyn SinkStage>],
    audio_cons: &mut HeapCons<AudioBlock>,
    ctx: &mut PipelineContext,
    consumed_frames: &AtomicU64,
) -> Result<(), PipelineError> {
    while let Some(block) = audio_cons.try_pop() {
        write_block(sinks, &block, ctx)?;
        let channels = usize::from(block.channels.max(1));
        consumed_frames.fetch_add((block.samples.len() / channels) as u64, Ordering::AcqRel);
    }
    Ok(())
}

fn refresh_runtime_state(
    sinks: &mut [Box<dyn SinkStage>],
    ctx: &mut PipelineContext,
) -> Result<(), PipelineError> {
    for sink in sinks {
        sink.refresh_runtime_state(ctx)?;
    }
    Ok(())
}

fn stop_sinks(sinks: &mut [Box<dyn SinkStage>], ctx: &mut PipelineContext) {
    for sink in sinks {
        sink.stop(ctx);
    }
}

/// Handles one sink-control command.
///
/// Returns `true` when the sink thread should exit its main loop.
fn handle_control(
    control: SinkControl,
    sinks: &mut [Box<dyn SinkStage>],
    audio_cons: &mut HeapCons<AudioBlock>,
    ctx: &mut PipelineContext,
    consumed_frames: &AtomicU64,
) -> ControlOutcome {
    match control {
        SinkControl::RefreshRuntimeState {
            ctx: next_ctx,
            resp_tx,
        } => {
            // Caller sends a full context snapshot; sink thread adopts it atomically.
            *ctx = next_ctx;
            let _ = resp_tx.send(refresh_runtime_state(sinks, ctx));
            ControlOutcome {
                break_loop: false,
                sinks_stopped: false,
            }
        },
        SinkControl::Drain { resp_tx } => {
            let result = drain_audio_ring_to_sinks(sinks, audio_cons, ctx, consumed_frames)
                .and_then(|_| flush_sinks(sinks, ctx));
            let _ = resp_tx.send(result);
            ControlOutcome {
                break_loop: false,
                sinks_stopped: false,
            }
        },
        SinkControl::DropQueued { resp_tx } => {
            let _ = audio_cons.clear();
            let _ = resp_tx.send(Ok(()));
            ControlOutcome {
                break_loop: false,
                sinks_stopped: false,
            }
        },
        SinkControl::Shutdown { drain, resp_tx } => {
            if !drain {
                let _ = audio_cons.clear();
            } else {
                let _ = drain_audio_ring_to_sinks(sinks, audio_cons, ctx, consumed_frames);
                let _ = flush_sinks(sinks, ctx);
            }
            stop_sinks(sinks, ctx);
            let _ = resp_tx.send(Ok(()));
            ControlOutcome {
                break_loop: true,
                sinks_stopped: true,
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::workers::sink::worker::SinkWorker;
    use crate::workers::sink::worker::SinkWriteError;
    use std::time::Duration;
    use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
    use stellatune_audio_core::pipeline::error::PipelineError;
    use stellatune_audio_core::pipeline::stages::sink::SinkStage;
    use stellatune_audio_core::pipeline::stages::{Stage, StageFlow};

    struct FatalOnWriteSink;
    struct AcceptingSink;

    impl Stage for AcceptingSink {}

    impl SinkStage for AcceptingSink {
        fn prepare(
            &mut self,
            _spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn write(
            &mut self,
            _block: &AudioBlock,
            _ctx: &mut PipelineContext,
        ) -> Result<StageFlow, PipelineError> {
            Ok(StageFlow::Continue)
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    impl Stage for FatalOnWriteSink {}

    impl SinkStage for FatalOnWriteSink {
        fn prepare(
            &mut self,
            _spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }
        fn write(
            &mut self,
            _block: &AudioBlock,
            _ctx: &mut PipelineContext,
        ) -> Result<StageFlow, PipelineError> {
            Err(PipelineError::StageFailure("fatal test sink".to_string()))
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    #[test]
    fn control_calls_report_sink_disconnected_after_sink_loop_exit() {
        let mut worker = SinkWorker::start(
            vec![Box::new(FatalOnWriteSink)],
            StreamSpec {
                sample_rate: 48_000,
                channels: 2,
            },
            PipelineContext::default(),
            2,
        )
        .expect("sink worker should start");

        let block = AudioBlock {
            channels: 2,
            samples: vec![0.0, 0.0, 0.0, 0.0],
        };
        worker
            .try_send_block(block)
            .expect("first send should succeed");

        let mut disconnected = false;
        for _ in 0..30 {
            match worker
                .refresh_runtime_state(&PipelineContext::default(), Duration::from_millis(20))
            {
                Err(PipelineError::SinkDisconnected) => {
                    disconnected = true;
                    break;
                },
                Ok(()) | Err(PipelineError::StageFailure(_)) => {
                    std::thread::sleep(Duration::from_millis(10));
                },
                Err(other) => panic!("unexpected control error: {other:?}"),
            }
        }

        assert!(
            disconnected,
            "refresh_runtime_state should eventually return SinkDisconnected after sink loop exits"
        );

        match worker.try_send_block(AudioBlock {
            channels: 2,
            samples: vec![0.0, 0.0],
        }) {
            Err(SinkWriteError::Disconnected) => {},
            Err(SinkWriteError::Full(_)) => {
                panic!("expected disconnected after sink loop exit, got full queue")
            },
            Ok(()) => panic!("expected disconnected after sink loop exit"),
        }
    }

    #[test]
    fn consumed_position_tracks_written_frames_and_seek_origin() {
        let ctx = PipelineContext {
            position_ms: 500,
            ..PipelineContext::default()
        };
        let mut worker = SinkWorker::start(
            vec![Box::new(AcceptingSink)],
            StreamSpec {
                sample_rate: 1_000,
                channels: 2,
            },
            ctx,
            2,
        )
        .unwrap();
        worker
            .try_send_block(AudioBlock {
                channels: 2,
                samples: vec![0.0; 200],
            })
            .unwrap();
        worker.drain(Duration::from_millis(100)).unwrap();
        assert_eq!(worker.consumed_position_ms(), 600);

        worker.reset_consumed_position(2_000);
        worker
            .try_send_block(AudioBlock {
                channels: 2,
                samples: vec![0.0; 100],
            })
            .unwrap();
        worker.drain(Duration::from_millis(100)).unwrap();
        assert_eq!(worker.consumed_position_ms(), 2_050);
        worker.shutdown(false, Duration::from_millis(100)).unwrap();
    }
}
