use crate::engine::messages::RuntimeDspChainEntry;
use crate::engine::update_events::emit_config_update_runtime_event;
use crossbeam_channel::Receiver;
use stellatune_plugins::capabilities::dsp::DspInstance;
use stellatune_plugins::runtime::actor::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};
use stellatune_plugins::runtime::worker_endpoint::DspWorkerController;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DspStage {
    PreMix,
    PostMix,
}

pub(crate) struct ActiveDspNode {
    pub(crate) spec: RuntimeDspChainEntry,
    pub(crate) stage: DspStage,
    pub(crate) controller: DspWorkerController,
    pub(crate) control_rx: Receiver<WorkerControlMessage>,
}

pub(crate) fn apply_dsp_stage(
    chain: &mut [ActiveDspNode],
    stage: DspStage,
    samples: &mut [f32],
    out_channels: usize,
) {
    if chain.is_empty() || out_channels == 0 {
        return;
    }
    let frames = (samples.len() / out_channels) as u32;
    if frames == 0 {
        return;
    }
    for node in chain.iter_mut() {
        if node.stage != stage {
            continue;
        }
        if let Some(instance) = node.controller.instance_mut() {
            instance.process_interleaved_f32_in_place(samples, frames);
        }
    }
}

pub(crate) fn layout_to_flag(channels: usize) -> u32 {
    use stellatune_plugin_api::{ST_LAYOUT_5_1, ST_LAYOUT_7_1, ST_LAYOUT_MONO, ST_LAYOUT_STEREO};
    match channels {
        1 => ST_LAYOUT_MONO,
        2 => ST_LAYOUT_STEREO,
        6 => ST_LAYOUT_5_1,
        8 => ST_LAYOUT_7_1,
        _ => ST_LAYOUT_STEREO,
    }
}

fn stage_for_instance(instance: &DspInstance, in_channels: usize) -> DspStage {
    let in_layout = layout_to_flag(in_channels);
    let supported = instance.supported_layouts();
    if supported == stellatune_plugin_api::ST_LAYOUT_ANY || (supported & in_layout) != 0 {
        DspStage::PreMix
    } else {
        DspStage::PostMix
    }
}

fn create_dsp_controller(
    spec: &RuntimeDspChainEntry,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<(DspWorkerController, Receiver<WorkerControlMessage>), String> {
    let endpoint = stellatune_plugins::runtime::handle::shared_runtime_service()
        .bind_dsp_worker_endpoint(
            &spec.plugin_id,
            &spec.type_id,
            target_sample_rate,
            target_channels,
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
        }
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => Err(format!(
            "failed to create DSP {}::{}: controller has no instance",
            spec.plugin_id, spec.type_id
        )),
    }
}

fn recreate_controller_instance(
    node: &mut ActiveDspNode,
    in_channels: usize,
) -> Result<(), String> {
    let previous_state = node
        .controller
        .instance()
        .and_then(|instance| instance.export_state_json().ok().flatten());

    node.controller.request_recreate();
    match node.controller.apply_pending().map_err(|e| e.to_string())? {
        WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated => {}
        WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle => {
            return Err(format!(
                "dsp recreate failed for {}::{}: controller has no instance",
                node.spec.plugin_id, node.spec.type_id
            ));
        }
    }

    if let Some(state_json) = previous_state
        && let Some(instance) = node.controller.instance_mut()
    {
        let _ = instance.import_state_json(&state_json);
    }

    let Some(instance) = node.controller.instance() else {
        return Err(format!(
            "dsp instance missing after recreate for {}::{}",
            node.spec.plugin_id, node.spec.type_id
        ));
    };
    node.stage = stage_for_instance(instance, in_channels);
    Ok(())
}

fn recreate_chain(
    chain: &[RuntimeDspChainEntry],
    in_channels: usize,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<Vec<ActiveDspNode>, String> {
    let mut out = Vec::with_capacity(chain.len());
    for spec in chain {
        let (controller, control_rx) =
            create_dsp_controller(spec, target_sample_rate, target_channels)?;
        let Some(instance) = controller.instance() else {
            return Err(format!(
                "failed to create DSP {}::{}: controller has no instance",
                spec.plugin_id, spec.type_id
            ));
        };
        let stage = stage_for_instance(instance, in_channels);
        out.push(ActiveDspNode {
            spec: spec.clone(),
            stage,
            controller,
            control_rx,
        });
    }
    Ok(out)
}

pub(crate) fn apply_or_recreate_dsp_chain(
    active: &mut Vec<ActiveDspNode>,
    desired: &[RuntimeDspChainEntry],
    in_channels: usize,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<(), String> {
    let same_shape = active.len() == desired.len()
        && active.iter().zip(desired.iter()).all(|(node, next)| {
            node.spec.plugin_id == next.plugin_id && node.spec.type_id == next.type_id
        });
    if !same_shape {
        *active = recreate_chain(desired, in_channels, target_sample_rate, target_channels)?;
        return Ok(());
    }

    for (node, next) in active.iter_mut().zip(desired.iter()) {
        if node.spec.config_json == next.config_json {
            continue;
        }

        let update_outcome = node
            .controller
            .apply_config_update(next.config_json.clone())
            .map_err(|e| {
                format!(
                    "dsp apply_config_update failed for {}::{}: {e}",
                    next.plugin_id, next.type_id
                )
            })?;

        match update_outcome {
            WorkerConfigUpdateOutcome::Applied {
                revision: generation,
            } => {
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "applied",
                    generation,
                    None,
                );
                node.spec.config_json = next.config_json.clone();
            }
            WorkerConfigUpdateOutcome::RequiresRecreate {
                revision: generation,
                reason,
            } => {
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "requires_recreate",
                    generation,
                    reason.as_deref(),
                );
                recreate_controller_instance(node, in_channels)?;
                node.spec.config_json = next.config_json.clone();
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "recreated",
                    generation,
                    None,
                );
            }
            WorkerConfigUpdateOutcome::DeferredNoInstance => {
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "requires_recreate",
                    0,
                    Some("dsp instance missing; deferred to recreate"),
                );
                recreate_controller_instance(node, in_channels)?;
                node.spec.config_json = next.config_json.clone();
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "recreated",
                    0,
                    Some("deferred_no_instance"),
                );
            }
            WorkerConfigUpdateOutcome::Rejected {
                revision: generation,
                reason,
            } => {
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "rejected",
                    generation,
                    Some(&reason),
                );
                return Err(format!(
                    "dsp config update rejected for {}::{}: {reason}",
                    next.plugin_id, next.type_id
                ));
            }
            WorkerConfigUpdateOutcome::Failed {
                revision: generation,
                error,
            } => {
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "failed",
                    generation,
                    Some(&error),
                );
                return Err(format!(
                    "dsp config update failed for {}::{}: {error}",
                    next.plugin_id, next.type_id
                ));
            }
        }

        let Some(instance) = node.controller.instance() else {
            return Err(format!(
                "dsp instance missing after update for {}::{}",
                next.plugin_id, next.type_id
            ));
        };
        node.stage = stage_for_instance(instance, in_channels);
    }
    Ok(())
}

pub(crate) fn apply_runtime_control_updates(
    active: &mut Vec<ActiveDspNode>,
    in_channels: usize,
    _target_sample_rate: u32,
    _target_channels: u16,
) -> Result<(), String> {
    let mut idx = 0usize;
    while idx < active.len() {
        while let Ok(msg) = active[idx].control_rx.try_recv() {
            active[idx].controller.on_control_message(msg);
        }

        if active[idx].controller.has_pending_destroy() {
            active.remove(idx);
            continue;
        }

        if active[idx].controller.has_pending_recreate() {
            recreate_controller_instance(&mut active[idx], in_channels)?;
        }

        idx = idx.saturating_add(1);
    }
    Ok(())
}
