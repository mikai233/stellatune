use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use crossbeam_channel::Sender;
use tracing::{debug, info, warn};

use stellatune_core::TrackDecodeInfo;
use stellatune_output::{OutputError, OutputHandle, OutputSpec};

use crate::engine::config::RING_BUFFER_CAPACITY_MS;
use crate::engine::decode::decode_thread;
use crate::engine::event_hub::EventHub;
use crate::engine::messages::{DecodeCtrl, InternalMsg};
use crate::ring_buffer::new_ring_buffer;

pub(crate) struct PlaybackSession {
    pub(crate) ctrl_tx: Sender<DecodeCtrl>,
    pub(crate) decode_join: JoinHandle<()>,
    pub(crate) _output: OutputHandle,
    pub(crate) output_enabled: Arc<AtomicBool>,
    pub(crate) volume: Arc<AtomicU32>,
    pub(crate) buffered_samples: Arc<AtomicUsize>,
    pub(crate) underrun_callbacks: Arc<AtomicU64>,
    pub(crate) out_sample_rate: u32,
    pub(crate) out_channels: u16,
    pub(crate) track_info: TrackDecodeInfo,
}

pub(crate) fn start_session(
    path: String,
    events: Arc<EventHub>,
    internal_tx: Sender<InternalMsg>,
    out_spec: OutputSpec,
    start_at_ms: i64,
    volume: Arc<AtomicU32>,
    plugins: Arc<Mutex<stellatune_plugins::PluginManager>>,
) -> Result<PlaybackSession, String> {
    let t0 = Instant::now();
    debug!(%path, start_at_ms, "start_session begin");
    info!("starting session");
    let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded();
    let (setup_tx, setup_rx) = crossbeam_channel::bounded::<DecodeCtrl>(1);
    let (spec_tx, spec_rx) = crossbeam_channel::bounded::<Result<TrackDecodeInfo, String>>(1);

    let thread_path = path.clone();
    let thread_events = Arc::clone(&events);
    let thread_internal_tx = internal_tx.clone();
    let thread_plugins = Arc::clone(&plugins);

    let decode_join = std::thread::Builder::new()
        .name("stellatune-decode".to_string())
        .spawn(move || {
            decode_thread(
                thread_path,
                thread_events,
                thread_internal_tx,
                thread_plugins,
                ctrl_rx,
                setup_rx,
                spec_tx,
            )
        })
        .expect("failed to spawn stellatune-decode thread");

    let t_after_spawn = Instant::now();
    let track_info = match spec_rx.recv() {
        Ok(Ok(info)) => info,
        Ok(Err(message)) => return Err(message),
        Err(_) => return Err("decoder thread exited unexpectedly".to_string()),
    };
    debug!(
        "decoder opened/probed in {}ms",
        t_after_spawn.elapsed().as_millis()
    );

    if out_spec.channels != 1 && out_spec.channels != 2 {
        warn!("unsupported output channels: {}", out_spec.channels);
        return Err(format!(
            "output channels = {}, only mono/stereo is supported",
            out_spec.channels
        ));
    }

    let capacity_samples =
        ((out_spec.sample_rate as usize * out_spec.channels as usize * RING_BUFFER_CAPACITY_MS)
            / 1000)
            .max(1024);
    let (producer, consumer) = new_ring_buffer::<f32>(capacity_samples);
    debug!(
        "ring buffer capacity: {} samples (~{}ms)",
        capacity_samples, RING_BUFFER_CAPACITY_MS
    );

    let output_enabled = Arc::new(AtomicBool::new(false));
    let buffered_samples = Arc::new(AtomicUsize::new(0));
    let underrun_callbacks = Arc::new(AtomicU64::new(0));
    let output_consumer = GatedConsumer {
        inner: consumer,
        enabled: Arc::clone(&output_enabled),
        volume: Arc::clone(&volume),
        buffered_samples: Arc::clone(&buffered_samples),
        underrun_callbacks: Arc::clone(&underrun_callbacks),
    };

    let output_internal_tx = internal_tx.clone();
    let t_output = Instant::now();
    let output = OutputHandle::start(output_consumer, out_spec.sample_rate, move |err| {
        let _ = output_internal_tx.try_send(InternalMsg::OutputError(err.to_string()));
    })
    .map_err(|e| match e {
        OutputError::ConfigMismatch { message } => message,
        other => other.to_string(),
    })?;
    debug!(
        "OutputHandle::start in {}ms",
        t_output.elapsed().as_millis()
    );

    setup_tx
        .send(DecodeCtrl::Setup {
            producer,
            target_sample_rate: out_spec.sample_rate,
            target_channels: out_spec.channels,
            start_at_ms,
            output_enabled: Arc::clone(&output_enabled),
        })
        .map_err(|_| "decoder thread exited unexpectedly".to_string())?;

    debug!("start_session total {}ms", t0.elapsed().as_millis());
    Ok(PlaybackSession {
        ctrl_tx,
        decode_join,
        _output: output,
        output_enabled,
        volume,
        buffered_samples,
        underrun_callbacks,
        out_sample_rate: out_spec.sample_rate,
        out_channels: out_spec.channels,
        track_info,
    })
}

struct GatedConsumer {
    inner: crate::ring_buffer::RingBufferConsumer<f32>,
    enabled: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    buffered_samples: Arc<AtomicUsize>,
    underrun_callbacks: Arc<AtomicU64>,
}

impl stellatune_output::SampleConsumer for GatedConsumer {
    fn pop_sample(&mut self) -> Option<f32> {
        if !self.enabled.load(Ordering::Acquire) {
            return None;
        }
        let s = self.inner.pop_sample()?;
        let v = f32::from_bits(self.volume.load(Ordering::Relaxed));
        Some(s * v)
    }

    fn on_output(&mut self, requested: usize, provided: usize) {
        self.buffered_samples
            .store(self.inner.len(), Ordering::Relaxed);
        if self.enabled.load(Ordering::Relaxed) && provided < requested {
            self.underrun_callbacks.fetch_add(1, Ordering::Relaxed);
        }
    }
}
