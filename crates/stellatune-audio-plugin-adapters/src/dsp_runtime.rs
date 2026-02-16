use crossbeam_channel::Receiver;
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;
use stellatune_plugins::runtime::worker_endpoint::DspWorkerController;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DspWorkerSpec {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
}

pub fn create_dsp_controller(
    spec: &DspWorkerSpec,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<(DspWorkerController, Receiver<WorkerControlMessage>), String> {
    let endpoint = stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service().bind_dsp_worker_endpoint(
            &spec.plugin_id,
            &spec.type_id,
            target_sample_rate,
            target_channels,
        ),
    )
    .map_err(|e| e.to_string())?;
    let (mut controller, control_rx) = endpoint.into_controller(spec.config_json.clone());
    match controller.apply_pending().map_err(|e| {
        format!(
            "failed to create DSP {}::{}: {e}",
            spec.plugin_id, spec.type_id
        )
    })? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok((controller, control_rx))
        },
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "failed to create DSP {}::{}: controller has no instance",
            spec.plugin_id, spec.type_id
        )),
    }
}

pub fn recreate_dsp_controller_instance(
    plugin_id: &str,
    type_id: &str,
    controller: &mut DspWorkerController,
) -> Result<(), String> {
    let previous_state = controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());

    controller.request_recreate();
    match controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "dsp recreate failed for {}::{}: controller has no instance",
                plugin_id, type_id
            ));
        },
    }

    if let Some(state_json) = previous_state
        && let Some(instance) = controller.instance_mut()
    {
        let _ = instance.import_state_json(&state_json);
    }

    if controller.instance().is_none() {
        return Err(format!(
            "dsp instance missing after recreate for {}::{}",
            plugin_id, type_id
        ));
    }

    Ok(())
}
