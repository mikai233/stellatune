use std::collections::HashSet;

use stellatune_audio_v2::assembly::PipelineMutation;
use stellatune_audio_v2::pipeline_graph::TransformGraphMutation;

use crate::v2_bridge::{
    ManagedPluginTransform, PluginTransformStageSpec, filter_transform_chain_specs_by_plugin_ids,
    plan_replace_managed_transform_chain,
};

#[derive(Debug, Clone, Default)]
pub struct PluginPipelineLifecycle {
    managed_stages: Vec<ManagedPluginTransform>,
}

impl PluginPipelineLifecycle {
    pub fn managed_stages(&self) -> &[ManagedPluginTransform] {
        &self.managed_stages
    }

    pub fn replace_transform_chain(
        &mut self,
        next: &[PluginTransformStageSpec],
    ) -> Vec<PipelineMutation> {
        let plan = plan_replace_managed_transform_chain(&self.managed_stages, next);
        self.managed_stages = plan.managed_stages.clone();
        plan.mutations
    }

    pub fn replace_transform_chain_filtered(
        &mut self,
        next: &[PluginTransformStageSpec],
        disabled_plugin_ids: &HashSet<String>,
    ) -> Vec<PipelineMutation> {
        let filtered = filter_transform_chain_specs_by_plugin_ids(next, disabled_plugin_ids);
        self.replace_transform_chain(&filtered)
    }

    pub fn remove_plugin(&mut self, plugin_id: &str) -> Vec<PipelineMutation> {
        self.remove_plugins(std::iter::once(plugin_id))
    }

    pub fn remove_plugins<'a, I>(&mut self, plugin_ids: I) -> Vec<PipelineMutation>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let remove_set = plugin_ids
            .into_iter()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect::<HashSet<_>>();
        if remove_set.is_empty() {
            return Vec::new();
        }

        let mut mutations = Vec::new();
        let mut removed_stage_keys = Vec::new();
        let mut kept = Vec::with_capacity(self.managed_stages.len());
        for stage in self.managed_stages.drain(..) {
            if remove_set.contains(stage.plugin_id.as_str()) {
                removed_stage_keys.push(stage.stage_key);
                continue;
            }
            kept.push(stage);
        }
        for target_stage_key in removed_stage_keys.into_iter().rev() {
            mutations.push(PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Remove { target_stage_key },
            });
        }
        self.managed_stages = kept;
        mutations
    }

    pub fn apply_runtime_deactivated_plugins<'a, I>(
        &mut self,
        deactivated_plugin_ids: I,
    ) -> Vec<PipelineMutation>
    where
        I: IntoIterator<Item = &'a str>,
    {
        self.remove_plugins(deactivated_plugin_ids)
    }

    pub fn clear_pipeline(&mut self) -> Vec<PipelineMutation> {
        let mut mutations = Vec::with_capacity(self.managed_stages.len());
        for stage in self.managed_stages.drain(..).rev() {
            mutations.push(PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Remove {
                    target_stage_key: stage.stage_key,
                },
            });
        }
        mutations
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::PluginPipelineLifecycle;
    use crate::v2_bridge::{PluginTransformSegment, PluginTransformStageSpec};
    use stellatune_audio_v2::assembly::PipelineMutation;
    use stellatune_audio_v2::pipeline_graph::TransformGraphMutation;

    #[test]
    fn remove_plugin_only_drops_matching_stages() {
        let mut lifecycle = PluginPipelineLifecycle::default();
        let _ = lifecycle.replace_transform_chain(&[
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
        ]);

        let mutations = lifecycle.remove_plugin("plugin-a");
        assert_eq!(mutations.len(), 1);
        match &mutations[0] {
            PipelineMutation::MutateTransformGraph {
                mutation: TransformGraphMutation::Remove { target_stage_key },
            } => assert!(target_stage_key.contains("plugin-a")),
            other => panic!("unexpected mutation: {other:?}"),
        }
        assert_eq!(lifecycle.managed_stages().len(), 1);
        assert_eq!(lifecycle.managed_stages()[0].plugin_id, "plugin-b");
    }

    #[test]
    fn apply_runtime_deactivated_plugins_removes_all_deactivated_plugins() {
        let mut lifecycle = PluginPipelineLifecycle::default();
        let _ = lifecycle.replace_transform_chain(&[
            PluginTransformStageSpec {
                plugin_id: "plugin-a".to_string(),
                type_id: "eq".to_string(),
                config_json: "{}".to_string(),
                segment: PluginTransformSegment::PreMix,
            },
            PluginTransformStageSpec {
                plugin_id: "plugin-b".to_string(),
                type_id: "limiter".to_string(),
                config_json: "{}".to_string(),
                segment: PluginTransformSegment::Main,
            },
            PluginTransformStageSpec {
                plugin_id: "plugin-c".to_string(),
                type_id: "clipper".to_string(),
                config_json: "{}".to_string(),
                segment: PluginTransformSegment::PostMix,
            },
        ]);

        let deactivated = vec!["plugin-b", "plugin-c"];
        let mutations = lifecycle.apply_runtime_deactivated_plugins(deactivated);
        assert_eq!(mutations.len(), 2);
        assert_eq!(lifecycle.managed_stages().len(), 1);
        assert_eq!(lifecycle.managed_stages()[0].plugin_id, "plugin-a");
    }

    #[test]
    fn replace_transform_chain_filtered_skips_disabled_plugins() {
        let mut lifecycle = PluginPipelineLifecycle::default();
        let mut disabled = HashSet::new();
        disabled.insert("plugin-b".to_string());

        let mutations = lifecycle.replace_transform_chain_filtered(
            &[
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
            ],
            &disabled,
        );

        assert_eq!(mutations.len(), 1);
        assert_eq!(lifecycle.managed_stages().len(), 1);
        assert_eq!(lifecycle.managed_stages()[0].plugin_id, "plugin-a");
    }
}
