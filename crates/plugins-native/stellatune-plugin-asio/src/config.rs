use serde::{Deserialize, Serialize};
use stellatune_asio_proto::{AudioSpec, DeviceCaps};
use stellatune_plugin_sdk::{
    ST_OUTPUT_NEGOTIATE_CHANGED_CH, ST_OUTPUT_NEGOTIATE_CHANGED_SR, ST_OUTPUT_NEGOTIATE_EXACT,
    ST_OUTPUT_NEGOTIATE_PREFER_TRACK_RATE, SdkError, SdkResult, StAudioSpec, StLogLevel,
    StOutputSinkNegotiatedSpec, host_log,
};

pub(crate) const CONFIG_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "sidecar_path": { "type": "string" },
    "sidecar_args": {
      "type": "array",
      "items": { "type": "string" },
      "default": []
    },
    "buffer_size_frames": { "type": ["integer", "null"], "minimum": 16 },
    "sample_rate_mode": {
      "type": "string",
      "enum": ["fixed_target", "match_track"],
      "default": "fixed_target",
      "title": "Sample Rate Mode",
      "description": "fixed_target: keep one output sample rate (best for lessgap). match_track: follow each track sample rate."
    },
    "fixed_target_sample_rate": {
      "type": ["integer", "null"],
      "minimum": 8000,
      "title": "Fixed Target Sample Rate",
      "description": "Used when sample_rate_mode=fixed_target. null means host desired sample rate."
    },
    "ring_capacity_ms": { "type": "integer", "minimum": 20, "default": 40 },
    "start_prefill_ms": {
      "type": "integer",
      "minimum": 0,
      "default": 0,
      "title": "Start Prefill (ms)",
      "description": "ASIO sidecar stream starts only after this much audio is buffered in shared ring. 0 means auto by Latency Profile."
    },
    "preferred_chunk_frames": {
      "type": "integer",
      "minimum": 0,
      "default": 0,
      "title": "Preferred Chunk Frames",
      "description": "0 means auto-tune by sample rate (recommended). >0 uses fixed chunk size."
    },
    "latency_profile": {
      "type": "string",
      "enum": ["aggressive", "balanced", "conservative"],
      "default": "aggressive",
      "title": "Latency Profile",
      "description": "Controls auto chunk size and auto prefill threshold when manual overrides are 0."
    },
    "flush_timeout_ms": { "type": "integer", "minimum": 1, "default": 400 }
  }
}"#;

pub(crate) const OUTPUT_SINK_TYPE_ID: &str = "asio";
pub(crate) const OUTPUT_SINK_DISPLAY_NAME: &str = "ASIO (Sidecar)";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AsioSampleRateMode {
    #[default]
    FixedTarget,
    MatchTrack,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AsioLatencyProfile {
    #[default]
    Aggressive,
    Balanced,
    Conservative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AsioOutputConfig {
    pub sidecar_path: Option<String>,
    pub sidecar_args: Vec<String>,
    pub buffer_size_frames: Option<u32>,
    pub sample_rate_mode: AsioSampleRateMode,
    pub fixed_target_sample_rate: Option<u32>,
    pub ring_capacity_ms: u32,
    pub start_prefill_ms: u32,
    pub preferred_chunk_frames: u32,
    pub latency_profile: AsioLatencyProfile,
    pub flush_timeout_ms: u64,
}

impl Default for AsioOutputConfig {
    fn default() -> Self {
        Self {
            sidecar_path: None,
            sidecar_args: Vec::new(),
            buffer_size_frames: None,
            sample_rate_mode: AsioSampleRateMode::FixedTarget,
            fixed_target_sample_rate: None,
            ring_capacity_ms: 40,
            start_prefill_ms: 0,
            preferred_chunk_frames: 0,
            latency_profile: AsioLatencyProfile::Aggressive,
            flush_timeout_ms: 400,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AsioOutputTarget {
    pub id: String,
    pub name: Option<String>,
    #[serde(default)]
    pub selection_session_id: Option<String>,
}

impl AsioOutputTarget {
    pub(crate) fn required_selection_session_id(&self) -> SdkResult<&str> {
        let session_id = self
            .selection_session_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                SdkError::invalid_arg(
                    "ASIO output target is missing selection_session_id. Refresh output sink targets before apply.",
                )
            })?;
        Ok(session_id)
    }
}

pub(crate) fn build_negotiated_spec(
    desired_spec: StAudioSpec,
    caps: &DeviceCaps,
    config: &AsioOutputConfig,
) -> StOutputSinkNegotiatedSpec {
    let desired_sr = desired_spec.sample_rate.max(1);
    let desired_ch = desired_spec.channels.max(1);

    let sample_rate = choose_sample_rate(desired_sr, caps, config);
    let channels = choose_channels(desired_ch, caps);
    host_log(
        StLogLevel::Debug,
        &format!(
            "asio negotiate mode={:?} latency={:?} desired={}Hz/{}ch chosen={}Hz/{}ch chunk={}f",
            config.sample_rate_mode,
            config.latency_profile,
            desired_sr,
            desired_ch,
            sample_rate,
            channels,
            preferred_chunk_frames(sample_rate, config)
        ),
    );

    let mut flags = 0u32;
    if sample_rate != desired_sr {
        flags |= ST_OUTPUT_NEGOTIATE_CHANGED_SR;
    }
    if channels != desired_ch {
        flags |= ST_OUTPUT_NEGOTIATE_CHANGED_CH;
    }
    if matches!(config.sample_rate_mode, AsioSampleRateMode::MatchTrack) {
        flags |= ST_OUTPUT_NEGOTIATE_PREFER_TRACK_RATE;
    }
    if flags == 0 {
        flags |= ST_OUTPUT_NEGOTIATE_EXACT;
    }

    StOutputSinkNegotiatedSpec {
        spec: StAudioSpec {
            sample_rate,
            channels,
            reserved: 0,
        },
        preferred_chunk_frames: preferred_chunk_frames(sample_rate, config),
        flags,
        reserved: 0,
    }
}

pub(crate) fn choose_sample_rate(
    desired: u32,
    caps: &DeviceCaps,
    config: &AsioOutputConfig,
) -> u32 {
    match config.sample_rate_mode {
        AsioSampleRateMode::FixedTarget => match config.fixed_target_sample_rate {
            Some(rate) => {
                let rate = rate.max(1);
                if !caps.supported_sample_rates.is_empty()
                    && !caps.supported_sample_rates.contains(&rate)
                {
                    host_log(
                        StLogLevel::Warn,
                        &format!(
                            "asio fixed_target {}Hz not present in advertised caps, forcing exact target anyway",
                            rate
                        ),
                    );
                }
                rate
            },
            None => desired.max(1),
        },
        AsioSampleRateMode::MatchTrack => {
            let request = desired.max(1);
            choose_nearest_u32(
                request,
                &caps.supported_sample_rates,
                caps.default_spec.sample_rate,
            )
        },
    }
}

pub(crate) fn startup_prefill_samples(spec: &AudioSpec, config: &AsioOutputConfig) -> usize {
    let channels = spec.channels.max(1) as usize;
    let sr = spec.sample_rate.max(1) as u64;
    let prefill_ms = effective_start_prefill_ms(config) as u64;
    let prefill_samples = sr
        .saturating_mul(channels as u64)
        .saturating_mul(prefill_ms)
        / 1000;
    let min_frames = config
        .buffer_size_frames
        .unwrap_or(preferred_chunk_frames(spec.sample_rate, config).max(128))
        .max(1) as u64;
    let min_samples = min_frames.saturating_mul(channels as u64);
    prefill_samples.max(min_samples).min(usize::MAX as u64) as usize
}

pub(crate) fn preferred_chunk_frames(sample_rate: u32, config: &AsioOutputConfig) -> u32 {
    if config.preferred_chunk_frames > 0 {
        return config.preferred_chunk_frames.max(1);
    }
    auto_preferred_chunk_frames(sample_rate, config)
}

pub(crate) fn auto_preferred_chunk_frames(sample_rate: u32, config: &AsioOutputConfig) -> u32 {
    let target = (sample_rate.max(1) / 375).max(64);
    let base = target.next_power_of_two().clamp(64, 1024);
    let scaled = match config.latency_profile {
        AsioLatencyProfile::Aggressive => (base / 2).max(64),
        AsioLatencyProfile::Balanced => base,
        AsioLatencyProfile::Conservative => base.saturating_mul(2),
    };
    scaled.clamp(64, 4096)
}

pub(crate) fn effective_start_prefill_ms(config: &AsioOutputConfig) -> u32 {
    if config.start_prefill_ms > 0 {
        return config.start_prefill_ms;
    }
    match config.latency_profile {
        AsioLatencyProfile::Aggressive => 8,
        AsioLatencyProfile::Balanced => 16,
        AsioLatencyProfile::Conservative => 32,
    }
}

pub(crate) fn choose_channels(desired: u16, caps: &DeviceCaps) -> u16 {
    choose_nearest_u16(
        desired.max(1),
        &caps.supported_channels,
        caps.default_spec.channels.max(1),
    )
}

pub(crate) fn choose_nearest_u32(desired: u32, supported: &[u32], fallback: u32) -> u32 {
    if supported.is_empty() {
        return fallback.max(1);
    }
    if supported.contains(&desired) {
        return desired;
    }

    let mut best = supported[0].max(1);
    let mut best_diff = desired.abs_diff(best);
    for &candidate in supported.iter().skip(1) {
        let candidate = candidate.max(1);
        let diff = desired.abs_diff(candidate);
        if diff < best_diff {
            best = candidate;
            best_diff = diff;
        }
    }
    best
}

pub(crate) fn choose_nearest_u16(desired: u16, supported: &[u16], fallback: u16) -> u16 {
    if supported.is_empty() {
        return fallback.max(1);
    }
    if supported.contains(&desired) {
        return desired;
    }

    let desired_u32 = desired as u32;
    let mut best = supported[0].max(1);
    let mut best_diff = desired_u32.abs_diff(best as u32);
    for &candidate in supported.iter().skip(1) {
        let candidate = candidate.max(1);
        let diff = desired_u32.abs_diff(candidate as u32);
        if diff < best_diff {
            best = candidate;
            best_diff = diff;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellatune_asio_proto::{AudioSpec, DeviceCaps, SampleFormat};

    fn test_caps() -> DeviceCaps {
        DeviceCaps {
            default_spec: AudioSpec {
                sample_rate: 48_000,
                channels: 2,
            },
            supported_sample_rates: vec![44_100, 48_000, 96_000],
            supported_channels: vec![2],
            supported_formats: vec![SampleFormat::F32],
        }
    }

    #[test]
    fn build_negotiated_spec_sets_prefer_track_rate_for_match_track() {
        let config = AsioOutputConfig {
            sample_rate_mode: AsioSampleRateMode::MatchTrack,
            ..Default::default()
        };
        let negotiated = build_negotiated_spec(
            StAudioSpec {
                sample_rate: 44_100,
                channels: 2,
                reserved: 0,
            },
            &test_caps(),
            &config,
        );

        assert_ne!(negotiated.flags & ST_OUTPUT_NEGOTIATE_PREFER_TRACK_RATE, 0);
    }

    #[test]
    fn build_negotiated_spec_does_not_set_prefer_track_rate_for_fixed_target() {
        let config = AsioOutputConfig::default();
        let negotiated = build_negotiated_spec(
            StAudioSpec {
                sample_rate: 48_000,
                channels: 2,
                reserved: 0,
            },
            &test_caps(),
            &config,
        );

        assert_eq!(negotiated.flags & ST_OUTPUT_NEGOTIATE_PREFER_TRACK_RATE, 0);
        assert_ne!(negotiated.flags & ST_OUTPUT_NEGOTIATE_EXACT, 0);
    }

    #[test]
    fn choose_sample_rate_fixed_target_none_uses_desired_rate() {
        let config = AsioOutputConfig {
            sample_rate_mode: AsioSampleRateMode::FixedTarget,
            fixed_target_sample_rate: None,
            ..Default::default()
        };
        let caps = DeviceCaps {
            default_spec: AudioSpec {
                sample_rate: 44_100,
                channels: 2,
            },
            supported_sample_rates: vec![44_100, 48_000, 96_000],
            supported_channels: vec![2],
            supported_formats: vec![SampleFormat::F32],
        };

        assert_eq!(choose_sample_rate(48_000, &caps, &config), 48_000);
    }
}
