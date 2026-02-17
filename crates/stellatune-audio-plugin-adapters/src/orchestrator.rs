use std::collections::HashSet;

use stellatune_audio::assembly::PipelineMutation;
use stellatune_audio::control::EngineHandle;

use crate::lifecycle::PluginPipelineLifecycle;
use crate::bridge::PluginTransformStageSpec;

#[derive(Debug, Clone, Default)]
pub struct PluginPipelineOrchestrator {
    lifecycle: PluginPipelineLifecycle,
}

impl PluginPipelineOrchestrator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lifecycle(&self) -> &PluginPipelineLifecycle {
        &self.lifecycle
    }

    pub fn into_lifecycle(self) -> PluginPipelineLifecycle {
        self.lifecycle
    }

    pub fn replace_transform_chain_filtered_with<F>(
        &mut self,
        next: &[PluginTransformStageSpec],
        disabled_plugin_ids: &HashSet<String>,
        mut apply: F,
    ) -> Result<usize, String>
    where
        F: FnMut(PipelineMutation) -> Result<(), String>,
    {
        let mut next_lifecycle = self.lifecycle.clone();
        let mutations = next_lifecycle.replace_transform_chain_filtered(next, disabled_plugin_ids);
        for mutation in mutations.iter().cloned() {
            apply(mutation)?;
        }
        self.lifecycle = next_lifecycle;
        Ok(mutations.len())
    }

    pub fn apply_runtime_deactivated_plugins_with<'a, I, F>(
        &mut self,
        deactivated_plugin_ids: I,
        mut apply: F,
    ) -> Result<usize, String>
    where
        I: IntoIterator<Item = &'a str>,
        F: FnMut(PipelineMutation) -> Result<(), String>,
    {
        let mut next_lifecycle = self.lifecycle.clone();
        let mutations = next_lifecycle.apply_runtime_deactivated_plugins(deactivated_plugin_ids);
        for mutation in mutations.iter().cloned() {
            apply(mutation)?;
        }
        self.lifecycle = next_lifecycle;
        Ok(mutations.len())
    }

    pub fn clear_pipeline_with<F>(&mut self, mut apply: F) -> Result<usize, String>
    where
        F: FnMut(PipelineMutation) -> Result<(), String>,
    {
        let mut next_lifecycle = self.lifecycle.clone();
        let mutations = next_lifecycle.clear_pipeline();
        for mutation in mutations.iter().cloned() {
            apply(mutation)?;
        }
        self.lifecycle = next_lifecycle;
        Ok(mutations.len())
    }

    pub async fn replace_transform_chain_filtered_on_engine(
        &mut self,
        engine: &EngineHandle,
        next: &[PluginTransformStageSpec],
        disabled_plugin_ids: &HashSet<String>,
    ) -> Result<usize, String> {
        let mut next_lifecycle = self.lifecycle.clone();
        let mutations = next_lifecycle.replace_transform_chain_filtered(next, disabled_plugin_ids);
        for mutation in mutations.iter().cloned() {
            engine.apply_pipeline_mutation(mutation).await?;
        }
        self.lifecycle = next_lifecycle;
        Ok(mutations.len())
    }

    pub async fn apply_runtime_deactivated_plugins_on_engine<'a, I>(
        &mut self,
        engine: &EngineHandle,
        deactivated_plugin_ids: I,
    ) -> Result<usize, String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut next_lifecycle = self.lifecycle.clone();
        let mutations = next_lifecycle.apply_runtime_deactivated_plugins(deactivated_plugin_ids);
        for mutation in mutations.iter().cloned() {
            engine.apply_pipeline_mutation(mutation).await?;
        }
        self.lifecycle = next_lifecycle;
        Ok(mutations.len())
    }

    pub async fn clear_pipeline_on_engine(
        &mut self,
        engine: &EngineHandle,
    ) -> Result<usize, String> {
        let mut next_lifecycle = self.lifecycle.clone();
        let mutations = next_lifecycle.clear_pipeline();
        for mutation in mutations.iter().cloned() {
            engine.apply_pipeline_mutation(mutation).await?;
        }
        self.lifecycle = next_lifecycle;
        Ok(mutations.len())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::bridge::{PluginTransformSegment, PluginTransformStageSpec};

    use super::PluginPipelineOrchestrator;

    fn stage(plugin_id: &str, type_id: &str) -> PluginTransformStageSpec {
        PluginTransformStageSpec {
            plugin_id: plugin_id.to_string(),
            type_id: type_id.to_string(),
            config_json: "{}".to_string(),
            segment: PluginTransformSegment::Main,
        }
    }

    #[test]
    fn replace_transform_chain_commits_state_after_successful_apply() {
        let mut orchestrator = PluginPipelineOrchestrator::new();
        let disabled = HashSet::new();
        let mut applied = Vec::new();

        let count = orchestrator
            .replace_transform_chain_filtered_with(
                &[stage("a", "eq"), stage("b", "limiter")],
                &disabled,
                |mutation| {
                    applied.push(mutation);
                    Ok(())
                },
            )
            .expect("replace should succeed");

        assert_eq!(count, 2);
        assert_eq!(applied.len(), 2);
        let managed = orchestrator.lifecycle().managed_stages();
        assert_eq!(managed.len(), 2);
        assert_eq!(managed[0].plugin_id, "a");
        assert_eq!(managed[1].plugin_id, "b");
    }

    #[test]
    fn replace_transform_chain_rolls_back_state_on_apply_failure() {
        let mut orchestrator = PluginPipelineOrchestrator::new();
        let disabled = HashSet::new();
        orchestrator
            .replace_transform_chain_filtered_with(&[stage("a", "eq")], &disabled, |_| Ok(()))
            .expect("seed replace should succeed");

        let error = orchestrator
            .replace_transform_chain_filtered_with(
                &[stage("b", "limiter")],
                &disabled,
                |_mutation| Err("inject failure".to_string()),
            )
            .expect_err("replace must fail");
        assert!(error.contains("inject failure"));

        let managed = orchestrator.lifecycle().managed_stages();
        assert_eq!(managed.len(), 1);
        assert_eq!(managed[0].plugin_id, "a");
    }

    #[test]
    fn apply_runtime_deactivated_plugins_commits_on_success() {
        let mut orchestrator = PluginPipelineOrchestrator::new();
        let disabled = HashSet::new();
        orchestrator
            .replace_transform_chain_filtered_with(
                &[stage("a", "eq"), stage("b", "limiter")],
                &disabled,
                |_| Ok(()),
            )
            .expect("seed replace should succeed");

        let mut removed = 0usize;
        let count = orchestrator
            .apply_runtime_deactivated_plugins_with(["b"], |_| {
                removed = removed.saturating_add(1);
                Ok(())
            })
            .expect("deactivate should succeed");

        assert_eq!(count, 1);
        assert_eq!(removed, 1);
        let managed = orchestrator.lifecycle().managed_stages();
        assert_eq!(managed.len(), 1);
        assert_eq!(managed[0].plugin_id, "a");
    }
}
