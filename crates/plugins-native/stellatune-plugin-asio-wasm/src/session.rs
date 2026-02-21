use std::thread;
use std::time::{Duration, Instant};

use stellatune_asio_proto::AudioSpec as ProtoAudioSpec;
use stellatune_wasm_plugin_sdk::prelude::*;

use crate::client::with_sidecar;
use crate::config::{
    AsioOutputConfig, AsioOutputTarget, build_negotiated_spec, startup_prefill_samples,
};

#[derive(Debug, Clone)]
struct CachedNegotiation {
    target_json: String,
    desired: AudioSpec,
    result: NegotiatedSpec,
}

pub struct AsioWasmSession {
    config: AsioOutputConfig,
    opened: bool,
    channels: u16,
    started: bool,
    start_prefill_samples: usize,
    flush_timeout_ms: u64,
    negotiated_cache: Option<CachedNegotiation>,
}

impl AsioWasmSession {
    pub fn new() -> SdkResult<Self> {
        Ok(Self {
            config: AsioOutputConfig::default(),
            opened: false,
            channels: 0,
            started: false,
            start_prefill_samples: 0,
            flush_timeout_ms: AsioOutputConfig::default().flush_timeout_ms.max(1),
            negotiated_cache: None,
        })
    }

    fn invalidate_negotiate_cache(&mut self) {
        self.negotiated_cache = None;
    }

    fn sdk_to_proto_spec(spec: &AudioSpec) -> ProtoAudioSpec {
        ProtoAudioSpec {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        }
    }
}

impl OutputSinkSession for AsioWasmSession {
    fn list_targets_json(&mut self) -> SdkResult<String> {
        let devices = with_sidecar(&self.config, |client| client.list_devices())?;
        let targets: Vec<AsioOutputTarget> = devices
            .into_iter()
            .map(|d| AsioOutputTarget {
                id: d.id,
                name: Some(d.name),
                selection_session_id: Some(d.selection_session_id),
            })
            .collect();

        serde_json::to_string(&targets)
            .map_err(|e| SdkError::internal(format!("serialize targets: {e}")))
    }

    fn negotiate_spec_json(
        &mut self,
        target_json: &str,
        desired: AudioSpec,
    ) -> SdkResult<NegotiatedSpec> {
        // Cache check
        if let Some(ref cached) = self.negotiated_cache
            && cached.target_json == target_json
            && cached.desired.sample_rate == desired.sample_rate
            && cached.desired.channels == desired.channels
        {
            return Ok(cached.result.clone());
        }

        let target: AsioOutputTarget = serde_json::from_str(target_json)
            .map_err(|e| SdkError::invalid_arg(format!("invalid target json: {e}")))?;
        let session_id = target.required_selection_session_id()?.to_string();

        let config = self.config.clone();
        let caps = with_sidecar(&self.config, |client| {
            client.get_device_caps(session_id, target.id.clone())
        })?;
        let result = build_negotiated_spec(desired, &caps, &config);

        self.negotiated_cache = Some(CachedNegotiation {
            target_json: target_json.to_string(),
            desired,
            result: result.clone(),
        });
        Ok(result)
    }

    fn describe_hot_path(&mut self, _spec: AudioSpec) -> SdkResult<Option<CoreModuleSpec>> {
        Ok(None)
    }

    fn open_json(&mut self, target_json: &str, spec: AudioSpec) -> SdkResult<()> {
        let target: AsioOutputTarget = serde_json::from_str(target_json)
            .map_err(|e| SdkError::invalid_arg(format!("invalid target json: {e}")))?;
        let session_id = target.required_selection_session_id()?.to_string();

        let proto_spec = Self::sdk_to_proto_spec(&spec);
        let start_prefill_samples =
            startup_prefill_samples(spec.sample_rate, spec.channels, &self.config);
        let flush_timeout_ms = self.config.flush_timeout_ms.max(1);
        let buffer_size_frames = self.config.buffer_size_frames;
        let queue_capacity_ms = Some(self.config.ring_capacity_ms.max(20));

        with_sidecar(&self.config, |client| {
            client.open(
                session_id,
                target.id,
                proto_spec,
                buffer_size_frames,
                queue_capacity_ms,
            )
        })?;

        self.opened = true;
        self.channels = spec.channels;
        self.started = false;
        self.start_prefill_samples = start_prefill_samples;
        self.flush_timeout_ms = flush_timeout_ms;
        self.invalidate_negotiate_cache();
        Ok(())
    }

    fn write_interleaved_f32(&mut self, channels: u16, interleaved_f32le: &[u8]) -> SdkResult<u32> {
        if !self.opened {
            return Err(SdkError::internal("output sink is not open"));
        }
        if channels == 0 {
            return Err(SdkError::invalid_arg("channels must be > 0"));
        }
        if channels != self.channels {
            return Err(SdkError::invalid_arg(format!(
                "channel mismatch: got {channels}, expected {}",
                self.channels
            )));
        }

        if !interleaved_f32le
            .len()
            .is_multiple_of(std::mem::size_of::<f32>())
        {
            return Err(SdkError::invalid_arg(
                "interleaved_f32le length must be a multiple of 4 bytes",
            ));
        }

        let channels_usize = channels as usize;
        let samples_len = interleaved_f32le.len() / std::mem::size_of::<f32>();
        if !samples_len.is_multiple_of(channels_usize) {
            return Err(SdkError::invalid_arg("samples not aligned to channels"));
        }

        let started = self.started;
        let start_prefill_samples = self.start_prefill_samples;
        let (frames_written, started_now) = with_sidecar(&self.config, |client| {
            let frames_written = client.write_samples(interleaved_f32le)?;
            let mut started_now = started;
            if !started_now && frames_written > 0 {
                let (queued_samples, running) = client.query_status().unwrap_or((0, false));
                if running {
                    started_now = true;
                } else if queued_samples as usize >= start_prefill_samples {
                    client.start()?;
                    started_now = true;
                }
            }
            Ok((frames_written, started_now))
        })?;
        self.started = started_now;

        Ok(frames_written)
    }

    fn query_status(&mut self) -> SdkResult<OutputSinkStatus> {
        if !self.opened {
            Ok(OutputSinkStatus {
                queued_samples: 0,
                running: false,
            })
        } else {
            match with_sidecar(&self.config, |client| client.query_status()) {
                Ok((queued_samples, running)) => Ok(OutputSinkStatus {
                    queued_samples,
                    running,
                }),
                Err(_) => Ok(OutputSinkStatus {
                    queued_samples: 0,
                    running: false,
                }),
            }
        }
    }

    fn flush(&mut self) -> SdkResult<()> {
        if !self.opened {
            return Ok(());
        }

        if !self.started {
            let (queued_samples, running) =
                with_sidecar(&self.config, |client| client.query_status())?;
            if running {
                self.started = true;
            } else if queued_samples > 0 {
                with_sidecar(&self.config, |client| client.start())?;
                self.started = true;
            }
        }

        let timeout = Duration::from_millis(self.flush_timeout_ms.max(1));
        let start = Instant::now();
        loop {
            let (queued_samples, _) = with_sidecar(&self.config, |client| client.query_status())?;
            if queued_samples == 0 {
                break;
            }
            if start.elapsed() >= timeout {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
        Ok(())
    }

    fn reset(&mut self) -> SdkResult<()> {
        if self.opened {
            let _ = with_sidecar(&self.config, |client| client.reset());
            self.started = false;
        }
        Ok(())
    }

    fn close(&mut self) -> SdkResult<()> {
        if self.opened {
            let _ = with_sidecar(&self.config, |client| {
                let _ = client.stop();
                let _ = client.reset();
                Ok(())
            });
        }
        self.opened = false;
        self.started = false;
        self.channels = 0;
        self.start_prefill_samples = 0;
        Ok(())
    }
}

impl ConfigStateOps for AsioWasmSession {
    fn plan_config_update_json(&mut self, _new_config_json: &str) -> SdkResult<ConfigUpdatePlan> {
        let mode = if self.opened {
            ConfigUpdateMode::Recreate
        } else {
            ConfigUpdateMode::HotApply
        };
        Ok(ConfigUpdatePlan { mode, reason: None })
    }

    fn apply_config_update_json(&mut self, new_config_json: &str) -> SdkResult<()> {
        let new_config: AsioOutputConfig = serde_json::from_str(new_config_json)
            .map_err(|e| SdkError::invalid_arg(format!("invalid config json: {e}")))?;
        self.config = new_config;
        self.flush_timeout_ms = self.config.flush_timeout_ms.max(1);
        self.invalidate_negotiate_cache();
        Ok(())
    }

    fn export_state_json(&self) -> SdkResult<Option<String>> {
        Ok(None)
    }

    fn import_state_json(&mut self, _state_json: &str) -> SdkResult<()> {
        Ok(())
    }
}
