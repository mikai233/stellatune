use crate::engine::messages::RuntimeDspChainEntry;
use crate::engine::update_events::emit_config_update_runtime_event;
use stellatune_plugins::DspInstance;
use stellatune_plugins::runtime::InstanceUpdateResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DspStage {
    PreMix,
    PostMix,
}

pub(crate) struct ActiveDspNode {
    pub(crate) spec: RuntimeDspChainEntry,
    pub(crate) stage: DspStage,
    pub(crate) instance: DspInstance,
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
        node.instance
            .process_interleaved_f32_in_place(samples, frames);
    }
}

pub(crate) fn layout_to_flag(channels: usize) -> u32 {
    use stellatune_plugin_api::*;
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

fn create_dsp_instance(
    spec: &RuntimeDspChainEntry,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<DspInstance, String> {
    let shared = stellatune_plugins::shared_runtime_service();
    let service = shared
        .lock()
        .map_err(|_| "plugin runtime v2 mutex poisoned".to_string())?;
    service
        .create_dsp_instance(
            &spec.plugin_id,
            &spec.type_id,
            target_sample_rate,
            target_channels,
            &spec.config_json,
        )
        .map_err(|e| {
            format!(
                "failed to create DSP {}::{}: {e}",
                spec.plugin_id, spec.type_id
            )
        })
}

fn recreate_chain(
    chain: &[RuntimeDspChainEntry],
    in_channels: usize,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<Vec<ActiveDspNode>, String> {
    let mut out = Vec::with_capacity(chain.len());
    for spec in chain {
        let instance = create_dsp_instance(spec, target_sample_rate, target_channels)?;
        let stage = stage_for_instance(&instance, in_channels);
        out.push(ActiveDspNode {
            spec: spec.clone(),
            stage,
            instance,
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
            .instance
            .apply_config_update_json(&next.config_json)
            .map_err(|e| {
                format!(
                    "dsp apply_config_update failed for {}::{}: {e}",
                    next.plugin_id, next.type_id
                )
            })?;

        match update_outcome {
            InstanceUpdateResult::Applied { generation, .. } => {
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
            InstanceUpdateResult::RequiresRecreate {
                generation, reason, ..
            } => {
                emit_config_update_runtime_event(
                    &next.plugin_id,
                    "dsp",
                    &next.type_id,
                    "requires_recreate",
                    generation,
                    reason.as_deref(),
                );
                let mut recreated =
                    match create_dsp_instance(next, target_sample_rate, target_channels) {
                        Ok(v) => v,
                        Err(error) => {
                            emit_config_update_runtime_event(
                                &next.plugin_id,
                                "dsp",
                                &next.type_id,
                                "failed",
                                generation,
                                Some(&error),
                            );
                            return Err(format!(
                                "dsp recreate failed for {}::{}: {error}",
                                next.plugin_id, next.type_id
                            ));
                        }
                    };
                if let Ok(Some(state_json)) = node.instance.export_state_json() {
                    let _ = recreated.import_state_json(&state_json);
                }
                node.instance = recreated;
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
            InstanceUpdateResult::Rejected {
                generation, reason, ..
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
            InstanceUpdateResult::Failed {
                generation, error, ..
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
        node.stage = stage_for_instance(&node.instance, in_channels);
    }
    Ok(())
}
