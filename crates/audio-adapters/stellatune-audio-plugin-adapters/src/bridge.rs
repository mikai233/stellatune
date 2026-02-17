use std::collections::HashSet;

use stellatune_audio::pipeline::assembly::{OpaqueTransformStageSpec, PipelineMutation};
use stellatune_audio::pipeline::graph::{
    TransformGraphMutation, TransformPosition, TransformSegment,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTransformSegment {
    PreMix,
    Main,
    PostMix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTransformStageSpec {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
    pub segment: PluginTransformSegment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTransformStagePayload {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPluginTransform {
    pub stage_key: String,
    pub plugin_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct TransformChainApplyPlan {
    pub mutations: Vec<PipelineMutation>,
    pub managed_stages: Vec<ManagedPluginTransform>,
}

pub fn plan_replace_managed_transform_chain(
    previous: &[ManagedPluginTransform],
    next: &[PluginTransformStageSpec],
) -> TransformChainApplyPlan {
    let mut mutations = Vec::with_capacity(previous.len().saturating_add(next.len()));

    for stage in previous.iter().rev() {
        mutations.push(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Remove {
                target_stage_key: stage.stage_key.clone(),
            },
        });
    }

    let mut managed_stages = Vec::with_capacity(next.len());
    let mut pre_mix_index = 0usize;
    let mut main_index = 0usize;
    let mut post_mix_index = 0usize;
    for spec in next {
        let (segment, ordinal) = match spec.segment {
            PluginTransformSegment::PreMix => {
                let index = pre_mix_index;
                pre_mix_index = pre_mix_index.saturating_add(1);
                (TransformSegment::PreMix, index)
            },
            PluginTransformSegment::Main => {
                let index = main_index;
                main_index = main_index.saturating_add(1);
                (TransformSegment::Main, index)
            },
            PluginTransformSegment::PostMix => {
                let index = post_mix_index;
                post_mix_index = post_mix_index.saturating_add(1);
                (TransformSegment::PostMix, index)
            },
        };

        let stage_key = make_plugin_transform_stage_key(spec, ordinal);
        let payload = PluginTransformStagePayload {
            plugin_id: spec.plugin_id.clone(),
            type_id: spec.type_id.clone(),
            config_json: spec.config_json.clone(),
        };
        mutations.push(PipelineMutation::MutateTransformGraph {
            mutation: TransformGraphMutation::Insert {
                segment,
                position: TransformPosition::Back,
                stage: OpaqueTransformStageSpec::with_payload(stage_key.clone(), payload),
            },
        });
        managed_stages.push(ManagedPluginTransform {
            stage_key,
            plugin_id: spec.plugin_id.clone(),
        });
    }

    TransformChainApplyPlan {
        mutations,
        managed_stages,
    }
}

pub fn filter_transform_chain_specs_by_plugin_ids(
    specs: &[PluginTransformStageSpec],
    excluded_plugin_ids: &HashSet<String>,
) -> Vec<PluginTransformStageSpec> {
    if excluded_plugin_ids.is_empty() {
        return specs.to_vec();
    }
    let excluded_normalized = excluded_plugin_ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .collect::<HashSet<_>>();
    specs
        .iter()
        .filter(|spec| !excluded_normalized.contains(spec.plugin_id.trim()))
        .cloned()
        .collect()
}

fn make_plugin_transform_stage_key(spec: &PluginTransformStageSpec, ordinal: usize) -> String {
    let segment = match spec.segment {
        PluginTransformSegment::PreMix => "pre",
        PluginTransformSegment::Main => "main",
        PluginTransformSegment::PostMix => "post",
    };
    let plugin_id = sanitize_stage_key_fragment(&spec.plugin_id);
    let type_id = sanitize_stage_key_fragment(&spec.type_id);
    format!("plugin.transform.{segment}.{plugin_id}.{type_id}.{ordinal}")
}

fn sanitize_stage_key_fragment(input: &str) -> String {
    input
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => ch,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        ManagedPluginTransform, PluginTransformSegment, PluginTransformStagePayload,
        PluginTransformStageSpec, filter_transform_chain_specs_by_plugin_ids,
        plan_replace_managed_transform_chain,
    };
    use stellatune_audio::pipeline::assembly::PipelineMutation;
    use stellatune_audio::pipeline::graph::{TransformGraphMutation, TransformSegment};

    #[test]
    fn replace_chain_generates_remove_then_insert_mutations() {
        let previous = vec![ManagedPluginTransform {
            stage_key: "plugin.transform.main.old.eq.0".to_string(),
            plugin_id: "old".to_string(),
        }];
        let next = vec![
            PluginTransformStageSpec {
                plugin_id: "a".to_string(),
                type_id: "eq".to_string(),
                config_json: "{}".to_string(),
                segment: PluginTransformSegment::PreMix,
            },
            PluginTransformStageSpec {
                plugin_id: "b".to_string(),
                type_id: "limiter".to_string(),
                config_json: "{}".to_string(),
                segment: PluginTransformSegment::Main,
            },
        ];

        let plan = plan_replace_managed_transform_chain(&previous, &next);
        assert_eq!(plan.mutations.len(), 3);
        match &plan.mutations[0] {
            PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Remove { target_stage_key },
            } => assert_eq!(target_stage_key, "plugin.transform.main.old.eq.0"),
            other => panic!("unexpected mutation[0]: {other:?}"),
        }
        match &plan.mutations[1] {
            PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Insert { segment, stage, .. },
            } => {
                assert_eq!(*segment, TransformSegment::PreMix);
                let payload = stage
                    .payload_ref::<PluginTransformStagePayload>()
                    .expect("payload must downcast");
                assert_eq!(payload.plugin_id, "a");
            },
            other => panic!("unexpected mutation[1]: {other:?}"),
        }
        assert_eq!(plan.managed_stages.len(), 2);
    }

    #[test]
    fn filter_transform_specs_excludes_disabled_plugins() {
        let specs = vec![
            PluginTransformStageSpec {
                plugin_id: "plugin-a".to_string(),
                type_id: "eq".to_string(),
                config_json: "{}".to_string(),
                segment: PluginTransformSegment::Main,
            },
            PluginTransformStageSpec {
                plugin_id: "plugin-b".to_string(),
                type_id: "limiter".to_string(),
                config_json: "{}".to_string(),
                segment: PluginTransformSegment::Main,
            },
        ];
        let excluded = HashSet::from(["plugin-b".to_string()]);
        let filtered = filter_transform_chain_specs_by_plugin_ids(&specs, &excluded);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].plugin_id, "plugin-a");
    }
}
