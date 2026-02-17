use std::time::{Duration, Instant};

use crossbeam_channel::Receiver;
use stellatune_plugin_api::StAudioSpec;
use stellatune_plugins::capabilities::output::OutputSinkInstance;
use stellatune_plugins::runtime::handle::shared_runtime_service;
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;
use stellatune_plugins::runtime::worker_endpoint::OutputSinkWorkerController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiatedOutputSinkSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

pub fn negotiate_output_sink_spec(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
    target_json: &str,
    desired_sample_rate: u32,
    desired_channels: u16,
) -> Result<NegotiatedOutputSinkSpec, String> {
    let endpoint = stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service()
            .bind_output_sink_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| format!("bind_output_sink_worker_endpoint failed: {e}"))?;
    let (mut controller, _control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "create_output_sink_instance failed for {}::{}: controller has no instance",
                plugin_id, type_id
            ));
        },
    }
    let Some(sink) = controller.instance_mut() else {
        return Err(format!(
            "create_output_sink_instance failed for {}::{}: controller has no instance",
            plugin_id, type_id
        ));
    };

    let negotiated = sink
        .negotiate_spec(
            target_json,
            StAudioSpec {
                sample_rate: desired_sample_rate.max(1),
                channels: desired_channels.max(1),
                reserved: 0,
            },
        )
        .map_err(|e| format!("output sink negotiate_spec failed: {e}"))?;
    Ok(NegotiatedOutputSinkSpec {
        sample_rate: negotiated.spec.sample_rate.max(1),
        channels: negotiated.spec.channels.max(1),
    })
}

pub fn recreate_output_sink_instance(
    plugin_id: &str,
    type_id: &str,
    target_json: &str,
    sample_rate: u32,
    channels: u16,
    controller: &mut OutputSinkWorkerController,
) -> Result<(), String> {
    let state_json = controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());
    controller.request_recreate();
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "output sink recreate failed for {}::{}: controller has no instance",
                plugin_id, type_id
            ));
        },
    }
    let Some(sink) = controller.instance_mut() else {
        return Err(format!(
            "output sink recreate failed for {}::{}: controller has no instance",
            plugin_id, type_id
        ));
    };
    sink.open(
        target_json,
        StAudioSpec {
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
            reserved: 0,
        },
    )
    .map_err(|e| format!("output sink reopen failed: {e}"))?;
    if let Some(state_json) = state_json {
        let _ = sink.import_state_json(&state_json);
    }
    Ok(())
}

pub fn create_output_sink_controller_and_open(
    plugin_id: &str,
    type_id: &str,
    config_json: &str,
    target_json: &str,
    sample_rate: u32,
    channels: u16,
) -> Result<(OutputSinkWorkerController, Receiver<WorkerControlMessage>), String> {
    let endpoint = stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service()
            .bind_output_sink_worker_endpoint(plugin_id, type_id),
    )
    .map_err(|e| format!("bind_output_sink_worker_endpoint failed: {e}"))?;
    let (mut controller, control_rx) = endpoint.into_controller(config_json.to_string());
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "create_output_sink_instance failed for {}::{}: controller has no instance",
                plugin_id, type_id
            ));
        },
    }
    let Some(sink) = controller.instance_mut() else {
        return Err(format!(
            "create_output_sink_instance failed for {}::{}: controller has no instance",
            plugin_id, type_id
        ));
    };
    sink.open(
        target_json,
        StAudioSpec {
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
            reserved: 0,
        },
    )
    .map_err(|e| format!("output sink reopen failed: {e}"))?;
    Ok((controller, control_rx))
}

pub fn current_plugin_lease_id(plugin_id: &str) -> u64 {
    stellatune_runtime::block_on(shared_runtime_service().current_plugin_lease_info(plugin_id))
        .map(|v| v.lease_id)
        .unwrap_or(0)
}

pub fn write_all_frames(
    sink: &mut OutputSinkInstance,
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
