use std::any::Any;

use crossbeam_channel::Receiver;
use stellatune_audio_core::pipeline::context::{AudioBlock, PipelineContext, StreamSpec};
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::StageStatus;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;
use stellatune_audio::assembly::OpaqueTransformStageSpec;
use stellatune_audio::pipeline_graph::TransformGraph;
use stellatune_plugins::runtime::messages::WorkerControlMessage;
use stellatune_plugins::runtime::worker_controller::{
    WorkerApplyPendingOutcome, WorkerConfigUpdateOutcome,
};
use stellatune_plugins::runtime::worker_endpoint::DspWorkerController as TransformWorkerController;

use crate::transform_runtime::{
    TransformWorkerSpec, apply_transform_controller_pending, bind_transform_controller,
    recreate_transform_controller_instance, sync_transform_runtime_control,
};
use crate::bridge::PluginTransformStagePayload;

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

pub struct PluginTransformStage {
    stage_key: String,
    worker_spec: TransformWorkerSpec,
    target_spec: Option<StreamSpec>,
    controller: Option<TransformWorkerController>,
    control_rx: Option<Receiver<WorkerControlMessage>>,
    last_runtime_error: Option<String>,
}

impl PluginTransformStage {
    pub fn new(stage_key: String, worker_spec: TransformWorkerSpec) -> Result<Self, String> {
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
            controller: None,
            control_rx: None,
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

    fn bind_controller(&mut self, spec: StreamSpec) -> Result<(), String> {
        let (controller, control_rx) = bind_transform_controller(
            &self.worker_spec,
            spec.sample_rate.max(1),
            spec.channels.max(1),
        )?;
        self.controller = Some(controller);
        self.control_rx = Some(control_rx);
        Ok(())
    }

    fn maybe_bind_controller(&mut self) {
        if self.controller.is_some() {
            return;
        }
        let Some(spec) = self.target_spec else {
            return;
        };
        if let Err(error) = self.bind_controller(spec) {
            self.last_runtime_error = Some(error);
        }
    }

    fn ensure_controller_strict(
        &mut self,
    ) -> Result<&mut TransformWorkerController, PipelineError> {
        if self.controller.is_none() {
            let spec = self.target_spec.ok_or_else(|| {
                PipelineError::StageFailure(format!(
                    "plugin transform '{}' is not prepared",
                    self.stage_key
                ))
            })?;
            self.bind_controller(spec)
                .map_err(PipelineError::StageFailure)?;
        }
        self.controller.as_mut().ok_or_else(|| {
            PipelineError::StageFailure(format!(
                "plugin transform '{}' has no controller",
                self.stage_key
            ))
        })
    }

    fn ensure_controller_instance_best_effort(&mut self) {
        let Some(controller) = self.controller.as_mut() else {
            return;
        };
        if controller.instance().is_some() && !controller.has_pending_recreate() {
            return;
        }
        match apply_transform_controller_pending(
            &self.worker_spec.plugin_id,
            &self.worker_spec.type_id,
            controller,
        ) {
            Ok(WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated) => {
                self.last_runtime_error = None;
            },
            Ok(WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle) => {},
            Err(error) => {
                self.last_runtime_error = Some(error);
            },
        }
    }

    fn sync_runtime_control_best_effort(&mut self) {
        self.maybe_bind_controller();
        self.ensure_controller_instance_best_effort();

        let Some(controller) = self.controller.as_mut() else {
            return;
        };
        let Some(control_rx) = self.control_rx.as_ref() else {
            self.last_runtime_error = Some(format!(
                "plugin transform {}::{} missing control receiver",
                self.worker_spec.plugin_id, self.worker_spec.type_id
            ));
            return;
        };

        match sync_transform_runtime_control(
            &self.worker_spec.plugin_id,
            &self.worker_spec.type_id,
            controller,
            control_rx,
        ) {
            Ok(WorkerApplyPendingOutcome::Created | WorkerApplyPendingOutcome::Recreated) => {
                self.last_runtime_error = None;
            },
            Ok(WorkerApplyPendingOutcome::Destroyed | WorkerApplyPendingOutcome::Idle) => {},
            Err(error) => {
                self.last_runtime_error = Some(error);
            },
        }
    }

    fn apply_config_control(
        &mut self,
        control: &PluginTransformConfigControl,
    ) -> Result<(), PipelineError> {
        let plugin_id = self.worker_spec.plugin_id.clone();
        let type_id = self.worker_spec.type_id.clone();
        self.worker_spec.config_json = control.config_json.clone();
        let controller = self.ensure_controller_strict()?;
        let outcome = controller
            .apply_config_update(control.config_json.clone())
            .map_err(|e| {
                PipelineError::StageFailure(format!(
                    "plugin transform config update failed for {}::{}: {e}",
                    plugin_id, type_id
                ))
            })?;
        match outcome {
            WorkerConfigUpdateOutcome::Applied { .. } => {
                self.last_runtime_error = None;
                Ok(())
            },
            WorkerConfigUpdateOutcome::RequiresRecreate { .. }
            | WorkerConfigUpdateOutcome::DeferredNoInstance => {
                recreate_transform_controller_instance(&plugin_id, &type_id, controller)
                    .map_err(PipelineError::StageFailure)?;
                self.last_runtime_error = None;
                Ok(())
            },
            WorkerConfigUpdateOutcome::Rejected { reason, .. } => {
                Err(PipelineError::StageFailure(format!(
                    "plugin transform config update rejected for {}::{}: {}",
                    plugin_id, type_id, reason
                )))
            },
            WorkerConfigUpdateOutcome::Failed { error, .. } => {
                Err(PipelineError::StageFailure(format!(
                    "plugin transform config update failed for {}::{}: {}",
                    plugin_id, type_id, error
                )))
            },
        }
    }

    fn apply_lifecycle_control(
        &mut self,
        control: PluginTransformLifecycleControl,
    ) -> Result<(), PipelineError> {
        let plugin_id = self.worker_spec.plugin_id.clone();
        let type_id = self.worker_spec.type_id.clone();
        let controller = self.ensure_controller_strict()?;
        match control.action {
            PluginTransformLifecycleAction::Recreate => {
                recreate_transform_controller_instance(&plugin_id, &type_id, controller)
                    .map_err(PipelineError::StageFailure)?;
            },
            PluginTransformLifecycleAction::Destroy => {
                controller.request_destroy();
                let _ = apply_transform_controller_pending(&plugin_id, &type_id, controller)
                    .map_err(PipelineError::StageFailure)?;
            },
        }
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
        self.bind_controller(spec)
            .map_err(PipelineError::StageFailure)?;
        self.ensure_controller_instance_best_effort();
        Ok(spec)
    }

    fn sync_runtime_control(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        self.sync_runtime_control_best_effort();
        Ok(())
    }

    fn process(&mut self, block: &mut AudioBlock, _ctx: &mut PipelineContext) -> StageStatus {
        if block.is_empty() {
            return StageStatus::Ok;
        }
        let Some(controller) = self.controller.as_mut() else {
            return StageStatus::Ok;
        };
        let Some(instance) = controller.instance_mut() else {
            return StageStatus::Ok;
        };
        let channels = block.channels.max(1) as usize;
        let frames = block.samples.len() / channels;
        if frames == 0 {
            return StageStatus::Ok;
        }
        instance.process_interleaved_f32_in_place(&mut block.samples, frames as u32);
        StageStatus::Ok
    }

    fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
        Ok(())
    }

    fn stop(&mut self, _ctx: &mut PipelineContext) {
        if let Some(controller) = self.controller.as_mut() {
            controller.request_destroy();
            let _ = apply_transform_controller_pending(
                &self.worker_spec.plugin_id,
                &self.worker_spec.type_id,
                controller,
            );
        }
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
    use stellatune_audio::assembly::OpaqueTransformStageSpec;
    use stellatune_audio::pipeline_graph::{TransformGraph, TransformGraphMutation};

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
                    segment: stellatune_audio::pipeline_graph::TransformSegment::PreMix,
                    position: stellatune_audio::pipeline_graph::TransformPosition::Back,
                    stage: OpaqueTransformStageSpec::with_payload(
                        "plugin.transform.pre.plugin-a.eq.0",
                        payload(&pre),
                    ),
                },
                TransformGraphMutation::Insert {
                    segment: stellatune_audio::pipeline_graph::TransformSegment::Main,
                    position: stellatune_audio::pipeline_graph::TransformPosition::Back,
                    stage: OpaqueTransformStageSpec::with_payload(
                        "plugin.transform.main.plugin-b.limiter.0",
                        payload(&main),
                    ),
                },
                TransformGraphMutation::Insert {
                    segment: stellatune_audio::pipeline_graph::TransformSegment::PostMix,
                    position: stellatune_audio::pipeline_graph::TransformPosition::Back,
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
