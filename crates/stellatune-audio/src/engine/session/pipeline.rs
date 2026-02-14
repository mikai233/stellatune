use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tracing::debug;

use stellatune_output::{OutputError, OutputHandle, OutputSpec};

use crate::engine::config::{RING_BUFFER_CAPACITY_MS, TRANSITION_FADE_RAMP_MS_TRACK_SWITCH};
use crate::engine::messages::InternalMsg;
use crate::ring_buffer::{RingBufferConsumer, new_ring_buffer};

use super::output_sink_worker::MasterGainProcessor;
use super::{OUTPUT_CONSUMER_CHUNK_SAMPLES, OutputPipeline};

pub(super) fn create_output_pipeline(
    backend: stellatune_output::AudioBackend,
    device_id: Option<String>,
    out_spec: OutputSpec,
    volume: Arc<AtomicU32>,
    internal_tx: crossbeam_channel::Sender<InternalMsg>,
    device_output_enabled: bool,
) -> Result<OutputPipeline, String> {
    let capacity_samples =
        ((out_spec.sample_rate as usize * out_spec.channels as usize * RING_BUFFER_CAPACITY_MS)
            / 1000)
            .max(1024);
    let (producer, consumer) = new_ring_buffer::<f32>(capacity_samples);
    let producer = Arc::new(Mutex::new(producer));
    let consumer = Arc::new(Mutex::new(consumer));

    let output_enabled = Arc::new(AtomicBool::new(false));
    let buffered_samples = Arc::new(AtomicUsize::new(0));
    let underrun_callbacks = Arc::new(AtomicU64::new(0));
    let transition_gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let transition_target_gain = Arc::new(AtomicU32::new(1.0f32.to_bits()));
    let transition_ramp_ms = Arc::new(AtomicU32::new(TRANSITION_FADE_RAMP_MS_TRACK_SWITCH as u32));

    let output = if device_output_enabled {
        let output_consumer = GatedConsumer {
            inner: Arc::clone(&consumer),
            enabled: Arc::clone(&output_enabled),
            master_gain: MasterGainProcessor::new(
                volume,
                Arc::clone(&transition_gain),
                Arc::clone(&transition_target_gain),
                Arc::clone(&transition_ramp_ms),
                out_spec.sample_rate,
            ),
            buffered_samples: Arc::clone(&buffered_samples),
            underrun_callbacks: Arc::clone(&underrun_callbacks),
            scratch: vec![0.0; OUTPUT_CONSUMER_CHUNK_SAMPLES],
            scratch_len: 0,
            scratch_cursor: 0,
            last_enabled: false,
        };

        let output = OutputHandle::start(
            backend,
            device_id.clone(),
            output_consumer,
            out_spec,
            move |err| {
                let _ = internal_tx.try_send(InternalMsg::OutputError(err.to_string()));
            },
        )
        .map_err(|e| match e {
            OutputError::ConfigMismatch { message } => message,
            other => other.to_string(),
        })?;
        Some(output)
    } else {
        None
    };

    Ok(OutputPipeline {
        _output: output,
        producer,
        output_enabled,
        buffered_samples,
        underrun_callbacks,
        transition_gain,
        transition_target_gain,
        transition_ramp_ms,
        backend,
        device_id,
        device_output_enabled,
        out_sample_rate: out_spec.sample_rate,
        out_channels: out_spec.channels,
    })
}

struct GatedConsumer {
    inner: Arc<Mutex<RingBufferConsumer<f32>>>,
    enabled: Arc<AtomicBool>,
    master_gain: MasterGainProcessor,
    buffered_samples: Arc<AtomicUsize>,
    underrun_callbacks: Arc<AtomicU64>,
    scratch: Vec<f32>,
    scratch_len: usize,
    scratch_cursor: usize,
    last_enabled: bool,
}

impl stellatune_output::SampleConsumer for GatedConsumer {
    fn pop_sample(&mut self) -> Option<f32> {
        let enabled = self.enabled.load(Ordering::Acquire);
        if !enabled {
            if self.last_enabled {
                let staged = self.scratch_len.saturating_sub(self.scratch_cursor);
                if staged > 0 {
                    debug!(
                        staged_samples = staged,
                        scratch_len = self.scratch_len,
                        scratch_cursor = self.scratch_cursor,
                        "output gate closed: dropping staged samples"
                    );
                } else {
                    debug!("output gate closed");
                }
                self.scratch_len = 0;
                self.scratch_cursor = 0;
                self.last_enabled = false;
            }
            return None;
        }
        if !self.last_enabled {
            debug!("output gate opened");
            self.last_enabled = true;
        }

        if self.scratch_cursor >= self.scratch_len {
            self.scratch_cursor = 0;
            self.scratch_len = if let Ok(mut inner) = self.inner.lock() {
                inner.pop_slice(&mut self.scratch)
            } else {
                0
            };
            if self.scratch_len == 0 {
                return None;
            }
        }

        let sample = self.scratch[self.scratch_cursor];
        self.scratch_cursor += 1;
        Some(self.master_gain.apply_sample(sample))
    }

    fn on_output(&mut self, requested: usize, provided: usize) {
        let buffered = self.inner.lock().map(|inner| inner.len()).unwrap_or(0);
        self.buffered_samples.store(buffered, Ordering::Relaxed);
        if self.enabled.load(Ordering::Relaxed) && provided < requested {
            self.underrun_callbacks.fetch_add(1, Ordering::Relaxed);
        }
    }
}
