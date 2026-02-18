use std::path::PathBuf;
use std::time::{Duration, Instant};

use stellatune_asio_proto::shm::SharedRingMapped;
use stellatune_asio_proto::{AudioSpec, Request};
use stellatune_plugin_sdk::{OutputSink, SdkError, SdkResult, StAudioSpec, StLogLevel, host_log};

use crate::client::{SidecarLease, acquire_sidecar_lease, sidecar_request_ok};
use crate::config::{AsioOutputConfig, startup_prefill_samples};
use crate::ring::create_ring;

const FLUSH_POLL_INTERVAL_MS: u64 = 2;

pub struct AsioOutputSink {
    config: AsioOutputConfig,
    _lease: SidecarLease,
    ring: SharedRingMapped,
    channels: u16,
    flush_timeout_ms: u64,
    started: bool,
    start_prefill_samples: usize,
    prefill_started_at: Instant,
    ring_path: PathBuf,
}

impl Drop for AsioOutputSink {
    fn drop(&mut self) {
        host_log(
            StLogLevel::Debug,
            &format!(
                "asio output sink drop: started={} queued_samples={} ring_path={}",
                self.started,
                self.queued_samples(),
                self.ring_path.display()
            ),
        );
        let _ = std::fs::remove_file(&self.ring_path);
    }
}

impl AsioOutputSink {
    pub(crate) fn open(
        spec: StAudioSpec,
        config: &AsioOutputConfig,
        target_id: String,
        selection_session_id: String,
    ) -> SdkResult<Self> {
        let lease = acquire_sidecar_lease(config)?;
        let spec = AudioSpec {
            sample_rate: spec.sample_rate.max(1),
            channels: spec.channels.max(1),
        };

        let (ring, ring_desc, ring_path) = create_ring(config.ring_capacity_ms, &spec)?;
        let open_result = sidecar_request_ok(
            config,
            Request::Open {
                selection_session_id: selection_session_id.clone(),
                device_id: target_id.clone(),
                spec: spec.clone(),
                buffer_size_frames: config.buffer_size_frames,
                shared_ring: Some(ring_desc),
            },
        );
        if let Err(e) = open_result {
            let _ = std::fs::remove_file(&ring_path);
            return Err(e);
        }
        host_log(
            StLogLevel::Debug,
            &format!(
                "asio output sink opened: target_id={:?} selection_session_id={:?} sample_rate={} channels={}",
                target_id, selection_session_id, spec.sample_rate, spec.channels
            ),
        );

        Ok(Self {
            config: config.clone(),
            _lease: lease,
            ring,
            channels: spec.channels,
            flush_timeout_ms: config.flush_timeout_ms.max(1),
            started: false,
            start_prefill_samples: startup_prefill_samples(&spec, config),
            prefill_started_at: Instant::now(),
            ring_path,
        })
    }

    fn maybe_start_sidecar(&mut self) -> SdkResult<()> {
        if self.started {
            return Ok(());
        }
        let buffered = self.ring.available_to_read();
        if buffered < self.start_prefill_samples {
            return Ok(());
        }
        sidecar_request_ok(&self.config, Request::Start)?;
        self.started = true;
        let prefill_elapsed_ms = self.prefill_started_at.elapsed().as_millis() as u64;
        host_log(
            StLogLevel::Debug,
            &format!(
                "asio sidecar stream started after prefill: buffered_samples={} threshold_samples={} prefill_elapsed_ms={}",
                buffered, self.start_prefill_samples, prefill_elapsed_ms
            ),
        );
        Ok(())
    }

    pub(crate) fn reset_for_disrupt(&mut self) -> SdkResult<()> {
        self.ring.reset();
        self.prefill_started_at = Instant::now();
        Ok(())
    }

    pub(crate) fn queued_samples(&self) -> u32 {
        self.ring.available_to_read().min(u32::MAX as usize) as u32
    }

    pub(crate) fn started(&self) -> bool {
        self.started
    }
}

impl OutputSink for AsioOutputSink {
    fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> SdkResult<u32> {
        if channels == 0 {
            return Err(SdkError::invalid_arg("channels must be > 0"));
        }
        if channels != self.channels {
            return Err(SdkError::invalid_arg(format!(
                "channel mismatch: got {channels}, expected {}",
                self.channels
            )));
        }
        let channels_usize = channels as usize;
        if !samples.len().is_multiple_of(channels_usize) {
            return Err(SdkError::invalid_arg("samples not aligned to channels"));
        }

        let accepted_samples = self.ring.write_samples(samples);
        self.maybe_start_sidecar()?;
        Ok((accepted_samples / channels_usize) as u32)
    }

    fn flush(&mut self) -> SdkResult<()> {
        if !self.started && self.ring.available_to_read() > 0 {
            sidecar_request_ok(&self.config, Request::Start)?;
            self.started = true;
            let prefill_elapsed_ms = self.prefill_started_at.elapsed().as_millis() as u64;
            host_log(
                StLogLevel::Debug,
                &format!(
                    "asio sidecar stream started on flush: prefill_elapsed_ms={prefill_elapsed_ms}"
                ),
            );
        }
        let timeout = Duration::from_millis(self.flush_timeout_ms.max(1));
        let start = Instant::now();
        while self.ring.available_to_read() > 0 {
            if start.elapsed() >= timeout {
                host_log(
                    StLogLevel::Warn,
                    &format!(
                        "asio sink flush timeout after {}ms (pending_samples={})",
                        self.flush_timeout_ms,
                        self.ring.available_to_read()
                    ),
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(FLUSH_POLL_INTERVAL_MS));
        }
        Ok(())
    }
}
