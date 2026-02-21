use std::any::Any;

use stellatune_audio::pipeline::assembly::OpaqueTransformStageSpec;
use stellatune_audio::pipeline::graph::TransformGraph;
use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;
use stellatune_wasm_plugins::host_runtime::{RuntimeDspPlugin, shared_runtime_service};

use crate::bridge::PluginTransformStagePayload;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransformWorkerSpec {
    plugin_id: String,
    type_id: String,
    config_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginTransformConfigControl {
    pub config_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTransformLifecycleAction {
    Recreate,
    Destroy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginTransformLifecycleControl {
    pub action: PluginTransformLifecycleAction,
}

struct RuntimeTransformInstance {
    plugin: RuntimeDspPlugin,
    spec: StreamSpec,
}

pub struct PluginTransformStage {
    stage_key: String,
    worker_spec: TransformWorkerSpec,
    target_spec: Option<StreamSpec>,
    instance: Option<RuntimeTransformInstance>,
    last_runtime_error: Option<String>,
}

impl PluginTransformStage {
    fn new(stage_key: String, worker_spec: TransformWorkerSpec) -> Result<Self, String> {
        let stage_key = stage_key.trim().to_string();
        if stage_key.is_empty() {
            return Err("plugin transform stage key must not be empty".to_string());
        }
        if worker_spec.plugin_id.trim().is_empty() {
            return Err(format!(
                "plugin transform stage '{}' plugin_id must not be empty",
                stage_key
            ));
        }
        if worker_spec.type_id.trim().is_empty() {
            return Err(format!(
                "plugin transform stage '{}' type_id must not be empty",
                stage_key
            ));
        }
        Ok(Self {
            stage_key,
            worker_spec,
            target_spec: None,
            instance: None,
            last_runtime_error: None,
        })
    }

    pub fn from_payload(stage: &OpaqueTransformStageSpec) -> Result<Self, String> {
        let payload = decode_plugin_transform_payload(stage)?;
        Self::new(
            stage.stage_key.clone(),
            TransformWorkerSpec {
                plugin_id: payload.plugin_id,
                type_id: payload.type_id,
                config_json: payload.config_json,
            },
        )
    }

    pub fn last_runtime_error(&self) -> Option<&str> {
        self.last_runtime_error.as_deref()
    }

    fn create_instance_for_spec(
        &self,
        spec: StreamSpec,
    ) -> Result<RuntimeTransformInstance, String> {
        let mut plugin = shared_runtime_service()
            .create_dsp_plugin(&self.worker_spec.plugin_id, &self.worker_spec.type_id)
            .map_err(|e| {
                format!(
                    "create_dsp_plugin failed for {}::{}: {e}",
                    self.worker_spec.plugin_id, self.worker_spec.type_id
                )
            })?;
        plugin
            .open_processor(spec.sample_rate.max(1), spec.channels.max(1))
            .map_err(|e| {
                format!(
                    "dsp open_processor failed for {}::{}: {e}",
                    self.worker_spec.plugin_id, self.worker_spec.type_id
                )
            })?;
        plugin
            .apply_config_update_json(&self.worker_spec.config_json)
            .map_err(|e| {
                format!(
                    "dsp apply_config_update_json failed for {}::{}: {e}",
                    self.worker_spec.plugin_id, self.worker_spec.type_id
                )
            })?;
        Ok(RuntimeTransformInstance { plugin, spec })
    }

    fn clear_instance(&mut self) {
        if let Some(mut instance) = self.instance.take() {
            let _ = instance.plugin.close_processor();
        }
    }

    fn recreate_instance_preserve_state(&mut self) -> Result<(), String> {
        let spec = self
            .target_spec
            .ok_or_else(|| format!("plugin transform '{}' is not prepared", self.stage_key))?;

        let state_json = self
            .instance
            .as_mut()
            .and_then(|instance| instance.plugin.export_state_json().ok().flatten());

        self.clear_instance();
        let mut next = self.create_instance_for_spec(spec)?;
        if let Some(state_json) = state_json {
            let _ = next.plugin.import_state_json(&state_json);
        }
        self.instance = Some(next);
        self.last_runtime_error = None;
        Ok(())
    }

    fn apply_config_control(
        &mut self,
        control: &PluginTransformConfigControl,
    ) -> Result<(), PipelineError> {
        self.worker_spec.config_json = control.config_json.clone();

        if self.instance.is_none() {
            return Ok(());
        }

        let apply_result = self
            .instance
            .as_mut()
            .ok_or_else(|| {
                PipelineError::StageFailure(format!(
                    "plugin transform '{}' instance unavailable",
                    self.stage_key
                ))
            })?
            .plugin
            .apply_config_update_json(&control.config_json);

        if let Err(error) = apply_result {
            self.recreate_instance_preserve_state()
                .map_err(PipelineError::StageFailure)?;
            return Err(PipelineError::StageFailure(format!(
                "plugin transform config update required recreate: {error}"
            )));
        }

        self.last_runtime_error = None;
        Ok(())
    }

    fn apply_lifecycle_control(
        &mut self,
        control: PluginTransformLifecycleControl,
    ) -> Result<(), PipelineError> {
        match control.action {
            PluginTransformLifecycleAction::Recreate => {
                self.recreate_instance_preserve_state()
                    .map_err(PipelineError::StageFailure)?;
            },
            PluginTransformLifecycleAction::Destroy => {
                self.clear_instance();
            },
        }
        self.last_runtime_error = None;
        Ok(())
    }
}

impl TransformStage for PluginTransformStage {
    fn stage_key(&self) -> Option<&str> {
        Some(self.stage_key.as_str())
    }

    fn apply_control(
        &mut self,
        control: &dyn Any,
        _ctx: &mut PipelineContext,
    ) -> Result<bool, PipelineError> {
        if let Some(control) = control.downcast_ref::<PluginTransformConfigControl>() {
            self.apply_config_control(control)?;
            return Ok(true);
        }
        if let Some(control) = control.downcast_ref::<PluginTransformLifecycleControl>() {
            self.apply_lifecycle_control(*control)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn prepare(
        &mut self,
        spec: StreamSpec,
        _ctx: &mut PipelineContext,
    ) -> Result<StreamSpec, PipelineError> {
        self.target_spec = Some(spec);
        self.clear_instance();
        let instance = self
            .create_instance_for_spec(spec)
            .map_err(PipelineError::StageFailure)?;
        self.instance = Some(instance);
        self.last_runtime_error = None;
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        if let Some(error) = self.last_runtime_error.take() {
            return Err(PipelineError::StageFailure(error));
        }
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if block.is_empty() {
            return StageStatus::Ok;
        }
        let Some(instance) = self.instance.as_mut() else {
            return StageStatus::Ok;
        };

        if instance.spec.channels != block.channels {
            self.last_runtime_error = Some(format!(
                "plugin transform '{}' channel mismatch: prepared={} block={}",
                self.stage_key, instance.spec.channels, block.channels
            ));
            return StageStatus::Fatal;
        }

        match instance
            .plugin
            .process_interleaved_f32_in_place(block.channels, &mut block.samples)
        {
            Ok(()) => StageStatus::Ok,
            Err(error) => {
                self.last_runtime_error = Some(format!(
                    "plugin transform '{}' process failed: {error}",
                    self.stage_key
                ));
                StageStatus::Fatal
            },
        }
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        self.clear_instance();
        self.target_spec = None;
        self.last_runtime_error = None;
    }
}

pub struct PluginTransformStageSet {
    pub pre_mix: Vec<Box<dyn TransformStage>>,
    pub main: Vec<Box<dyn TransformStage>>,
    pub post_mix: Vec<Box<dyn TransformStage>>,
}

pub fn decode_plugin_transform_payload(
    stage: &OpaqueTransformStageSpec,
) -> Result<PluginTransformStagePayload, String> {
    stage
        .payload_ref::<PluginTransformStagePayload>()
        .cloned()
        .ok_or_else(|| {
            format!(
                "expected PluginTransformStagePayload for stage '{}'",
                stage.stage_key
            )
        })
}

pub fn build_plugin_transform_stage(
    stage: &OpaqueTransformStageSpec,
) -> Result<Box<dyn TransformStage>, String> {
    let stage = PluginTransformStage::from_payload(stage)?;
    Ok(Box::new(stage))
}

pub fn build_plugin_transform_stage_set_from_graph(
    graph: &TransformGraph<OpaqueTransformStageSpec>,
) -> Result<PluginTransformStageSet, String> {
    let pre_mix = graph
        .pre_mix
        .iter()
        .map(build_plugin_transform_stage)
        .collect::<Result<Vec<_>, _>>()?;
    let main = graph
        .main
        .iter()
        .map(build_plugin_transform_stage)
        .collect::<Result<Vec<_>, _>>()?;
    let post_mix = graph
        .post_mix
        .iter()
        .map(build_plugin_transform_stage)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PluginTransformStageSet {
        pre_mix,
        main,
        post_mix,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        PluginTransformConfigControl, build_plugin_transform_stage_set_from_graph,
        decode_plugin_transform_payload,
    };
    use crate::bridge::{
        PluginTransformSegment, PluginTransformStagePayload, PluginTransformStageSpec,
    };
    use stellatune_audio::pipeline::assembly::OpaqueTransformStageSpec;
    use stellatune_audio::pipeline::graph::{TransformGraph, TransformGraphMutation};

    fn payload(spec: &PluginTransformStageSpec) -> PluginTransformStagePayload {
        PluginTransformStagePayload {
            plugin_id: spec.plugin_id.clone(),
            type_id: spec.type_id.clone(),
            config_json: spec.config_json.clone(),
        }
    }

    #[test]
    fn decode_payload_rejects_unexpected_payload_type() {
        let stage = OpaqueTransformStageSpec::with_payload("plugin.transform.main.eq.0", 42u64);
        let error = decode_plugin_transform_payload(&stage).expect_err("must fail");
        assert!(error.contains("expected PluginTransformStagePayload"));
    }

    #[test]
    fn build_stage_set_from_graph_keeps_segment_and_order() {
        let mut graph = TransformGraph::default();
        let pre = PluginTransformStageSpec {
            plugin_id: "plugin-a".to_string(),
            type_id: "eq".to_string(),
            config_json: "{}".to_string(),
            segment: PluginTransformSegment::PreMix,
        };
        let main = PluginTransformStageSpec {
            plugin_id: "plugin-b".to_string(),
            type_id: "limiter".to_string(),
            config_json: "{}".to_string(),
            segment: PluginTransformSegment::Main,
        };
        let post = PluginTransformStageSpec {
            plugin_id: "plugin-c".to_string(),
            type_id: "clipper".to_string(),
            config_json: "{}".to_string(),
            segment: PluginTransformSegment::PostMix,
        };
        graph
            .apply_mutations([
                TransformGraphMutation::Insert {
                    segment: stellatune_audio::pipeline::graph::TransformSegment::PreMix,
                    position: stellatune_audio::pipeline::graph::TransformPosition::Back,
                    stage: OpaqueTransformStageSpec::with_payload(
                        "plugin.transform.pre.plugin-a.eq.0",
                        payload(&pre),
                    ),
                },
                TransformGraphMutation::Insert {
                    segment: stellatune_audio::pipeline::graph::TransformSegment::Main,
                    position: stellatune_audio::pipeline::graph::TransformPosition::Back,
                    stage: OpaqueTransformStageSpec::with_payload(
                        "plugin.transform.main.plugin-b.limiter.0",
                        payload(&main),
                    ),
                },
                TransformGraphMutation::Insert {
                    segment: stellatune_audio::pipeline::graph::TransformSegment::PostMix,
                    position: stellatune_audio::pipeline::graph::TransformPosition::Back,
                    stage: OpaqueTransformStageSpec::with_payload(
                        "plugin.transform.post.plugin-c.clipper.0",
                        payload(&post),
                    ),
                },
            ])
            .expect("graph mutations must succeed");

        let set = build_plugin_transform_stage_set_from_graph(&graph).expect("build must succeed");
        assert_eq!(set.pre_mix.len(), 1);
        assert_eq!(set.main.len(), 1);
        assert_eq!(set.post_mix.len(), 1);
        assert_eq!(
            set.main[0].stage_key(),
            Some("plugin.transform.main.plugin-b.limiter.0")
        );
    }

    #[test]
    fn config_control_is_constructible() {
        let control = PluginTransformConfigControl {
            config_json: "{\"gain\":1}".to_string(),
        };
        assert_eq!(control.config_json, "{\"gain\":1}");
    }
}
