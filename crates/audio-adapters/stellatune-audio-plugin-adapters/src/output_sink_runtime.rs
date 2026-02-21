use std::time::{Duration, Instant};

use stellatune_plugins::host_runtime::{RuntimeOutputSinkPlugin, shared_runtime_service};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiatedOutputSinkSpec {
    pub sample_rate: u32,
    pub channels: u16,
    pub prefer_track_rate: bool,
}

pub fn negotiate_output_sink_spec(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
    target_json: &str,
    desired_sample_rate: u32,
    desired_channels: u16,
) -> Result<NegotiatedOutputSinkSpec, String> {
    let mut sink = shared_runtime_service()
        .create_output_sink_plugin(plugin_id, type_id)
        .map_err(|e| {
            format!(
                "create_output_sink_plugin failed for {}::{}: {e}",
                plugin_id, type_id
            )
        })?;
    sink.apply_config_update_json(config_json)
        .map_err(|e| format!("output sink apply_config_update_json failed: {e}"))?;

    let negotiated = sink
        .negotiate_spec(
            target_json,
            desired_sample_rate.max(1),
            desired_channels.max(1),
        )
        .map_err(|e| format!("output sink negotiate_spec_json failed: {e}"))?;

    Ok(NegotiatedOutputSinkSpec {
        sample_rate: negotiated.spec.sample_rate.max(1),
        channels: negotiated.spec.channels.max(1),
        prefer_track_rate: negotiated.prefer_track_rate,
    })
}

pub fn create_output_sink_controller_and_open(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
    target_json: &str,
    sample_rate: u32,
    channels: u16,
) -> Result<RuntimeOutputSinkPlugin, String> {
    let mut sink = shared_runtime_service()
        .create_output_sink_plugin(plugin_id, type_id)
        .map_err(|e| {
            format!(
                "create_output_sink_plugin failed for {}::{}: {e}",
                plugin_id, type_id
            )
        })?;
    sink.apply_config_update_json(config_json)
        .map_err(|e| format!("output sink apply_config_update_json failed: {e}"))?;
    sink.open(target_json, sample_rate.max(1), channels.max(1))
        .map_err(|e| format!("output sink open_json failed: {e}"))?;
    Ok(sink)
}

pub fn write_all_frames(
    sink: &mut RuntimeOutputSinkPlugin,
    channels: u16,
    samples: &[f32],
    retry_sleep_ms: u64,
    stall_timeout_ms: u64,
) -> Result<(), String> {
    let channels = channels.max(1) as usize;
    if channels == 0 || samples.is_empty() {
        return Ok(());
    }
    let mut offset = 0usize;
    let mut zero_accept_since: Option<Instant> = None;
    while offset < samples.len() {
        let frames_accepted = sink
            .write_interleaved_f32(channels as u16, &samples[offset..])
            .map_err(|e| e.to_string())?;
        let accepted_samples = frames_accepted as usize * channels;
        if accepted_samples == 0 {
            let started = *zero_accept_since.get_or_insert_with(Instant::now);
            if started.elapsed() >= Duration::from_millis(stall_timeout_ms) {
                let remaining_frames = (samples.len().saturating_sub(offset)) / channels;
                return Err(format!(
                    "output sink stalled: accepted 0 frames for {stall_timeout_ms}ms (remaining_frames={remaining_frames})"
                ));
            }
            std::thread::sleep(Duration::from_millis(retry_sleep_ms));
            continue;
        }
        zero_accept_since = None;
        offset = offset.saturating_add(accepted_samples.min(samples.len() - offset));
    }
    Ok(())
}
