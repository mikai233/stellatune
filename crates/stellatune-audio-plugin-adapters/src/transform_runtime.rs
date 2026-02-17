use crossbeam_channel::Receiver;
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::WorkerApplyPendingOutcome;
use stellatune_plugins::runtime::worker_endpoint::DspWorkerController as TransformWorkerController;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformWorkerSpec {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
}

pub fn bind_transform_controller(
    spec: &TransformWorkerSpec,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<(TransformWorkerController, Receiver<WorkerControlMessage>), String> {
    let endpoint = stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service().bind_dsp_worker_endpoint(
            &spec.plugin_id,
            &spec.type_id,
            target_sample_rate,
            target_channels,
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(endpoint.into_controller(spec.config_json.clone()))
}

pub fn apply_transform_controller_pending(
    plugin_id: &str,
    type_id: &str,
    controller: &mut TransformWorkerController,
) -> Result<WorkerApplyPendingOutcome, String> {
    controller.apply_pending().map_err(|e| {
        format!(
            "failed to apply pending transform {}::{}: {e}",
            plugin_id, type_id
        )
    })
}

pub fn sync_transform_runtime_control(
    plugin_id: &str,
    type_id: &str,
    controller: &mut TransformWorkerController,
    control_rx: &Receiver<WorkerControlMessage>,
) -> Result<WorkerApplyPendingOutcome, String> {
    while let Ok(message) = control_rx.try_recv() {
        controller.on_control_message(message);
    }

    if controller.has_pending_destroy() || controller.has_pending_recreate() {
        return apply_transform_controller_pending(plugin_id, type_id, controller);
    }

    Ok(WorkerApplyPendingOutcome::Idle)
}

pub fn create_transform_controller(
    spec: &TransformWorkerSpec,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<(TransformWorkerController, Receiver<WorkerControlMessage>), String> {
    let (mut controller, control_rx) =
        bind_transform_controller(spec, target_sample_rate, target_channels)?;
    match apply_transform_controller_pending(&spec.plugin_id, &spec.type_id, &mut controller)? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {
            Ok((controller, control_rx))
        },
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "failed to create transform {}::{}: controller has no instance",
            spec.plugin_id, spec.type_id
        )),
    }
}

pub fn recreate_transform_controller_instance(
    plugin_id: &str,
    type_id: &str,
    controller: &mut TransformWorkerController,
) -> Result<(), String> {
    let previous_state = controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());

    controller.request_recreate();
    match apply_transform_controller_pending(plugin_id, type_id, controller)? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {},
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "transform recreate failed for {}::{}: controller has no instance",
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
            "transform instance missing after recreate for {}::{}",
            plugin_id, type_id
        ));
    }

    Ok(())
}
