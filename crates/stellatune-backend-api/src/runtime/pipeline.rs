use std::any::Any;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use stellatune_audio::config::engine::LfeMode;
use stellatune_audio::pipeline::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlot, BuiltinTransformSlots,
    MixerPlan, OpaqueTransformStageSpec, PipelineAssembler, PipelineBlueprint, PipelineMutation,
    PipelineOutputBackend, PipelineRuntime, PipelineSinkRoute, ResamplerPlan, StaticSinkPlan,
    TransformChain,
};
use stellatune_audio::pipeline::graph::TransformGraph;
use stellatune_audio_builtin_adapters::device_sink::{
    DeviceSinkControl, DeviceSinkStage, OutputBackend,
};
use stellatune_audio_builtin_adapters::wasapi_exclusive_sink::WasapiExclusiveSinkStage;
use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_plugin_adapters::stages::{
    PluginOutputSinkRouteSpec, PluginOutputSinkStage, build_plugin_source,
    build_plugin_transform_stage_set_from_graph,
};

use super::engine::current_blueprint_output_spec;
use super::hybrid_decoder_stage::HybridDecoderStage;

const FALLBACK_OUTPUT_SAMPLE_RATE: u32 = 48_000;
const FALLBACK_OUTPUT_CHANNELS: u16 = 2;

fn fingerprint_builtin_output_route(backend: OutputBackend, device_id: Option<&str>) -> u64 {
    let mut hasher = DefaultHasher::new();
    "builtin_output_route".hash(&mut hasher);
    match backend {
        OutputBackend::Shared => 0_u8.hash(&mut hasher),
        OutputBackend::WasapiExclusive => 1_u8.hash(&mut hasher),
    }
    device_id.unwrap_or_default().hash(&mut hasher);
    hasher.finish()
}

fn fingerprint_plugin_output_route(route: &PluginOutputSinkRouteSpec) -> u64 {
    let mut hasher = DefaultHasher::new();
    "plugin_output_route".hash(&mut hasher);
    route.plugin_id.hash(&mut hasher);
    route.type_id.hash(&mut hasher);
    route.config_json.hash(&mut hasher);
    route.target_json.hash(&mut hasher);
    hasher.finish()
}

fn shared_sink_control_cell() -> &'static OnceLock<DeviceSinkControl> {
    static CONTROL: OnceLock<DeviceSinkControl> = OnceLock::new();
    &CONTROL
}

fn shared_sink_route_control_cell() -> &'static OnceLock<RuntimeSinkRouteControl> {
    static CONTROL: OnceLock<RuntimeSinkRouteControl> = OnceLock::new();
    &CONTROL
}

pub fn shared_device_sink_control() -> DeviceSinkControl {
    shared_sink_control_cell()
        .get_or_init(DeviceSinkControl::default)
        .clone()
}

pub fn shared_runtime_sink_route_control() -> RuntimeSinkRouteControl {
    shared_sink_route_control_cell()
        .get_or_init(RuntimeSinkRouteControl::default)
        .clone()
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSinkRouteControl {
    inner: Arc<Mutex<Option<PluginOutputSinkRouteSpec>>>,
}

impl RuntimeSinkRouteControl {
    pub fn set_plugin_route(&self, route: PluginOutputSinkRouteSpec) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(route);
        }
    }

    pub fn clear_plugin_route(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
    }

    pub fn current_plugin_route(&self) -> Option<PluginOutputSinkRouteSpec> {
        self.inner.lock().ok().and_then(|guard| guard.clone())
    }
}

#[derive(Debug, Clone)]
struct RuntimeBlueprint {
    transform_graph: TransformGraph<OpaqueTransformStageSpec>,
    mixer: Option<MixerPlan>,
    resampler: Option<ResamplerPlan>,
    builtin_slots: BuiltinTransformSlots,
    sink_route: PipelineSinkRoute,
}

#[derive(Debug, Clone)]
pub struct BackendAssembler {
    fallback_output_sample_rate: u32,
    fallback_output_channels: u16,
}

impl Default for BackendAssembler {
    fn default() -> Self {
        Self {
            fallback_output_sample_rate: FALLBACK_OUTPUT_SAMPLE_RATE,
            fallback_output_channels: FALLBACK_OUTPUT_CHANNELS,
        }
    }
}

impl PipelineAssembler for BackendAssembler {
    fn build_blueprint(
        &self,
        input: &InputRef,
    ) -> Result<Arc<dyn PipelineBlueprint>, PipelineError> {
        let InputRef::TrackToken(track_token) = input;
        if track_token.trim().is_empty() {
            return Err(PipelineError::StageFailure(
                "track token must not be empty".to_string(),
            ));
        }
        Ok(Arc::new(self.default_runtime_blueprint()))
    }

    fn apply_pipeline_mutation(
        &self,
        current: Option<&dyn PipelineBlueprint>,
        mutation: PipelineMutation,
    ) -> Result<Arc<dyn PipelineBlueprint>, PipelineError> {
        let mut blueprint = match current {
            Some(current) => (current as &dyn Any)
                .downcast_ref::<RuntimeBlueprint>()
                .cloned()
                .ok_or_else(|| {
                    PipelineError::StageFailure("unexpected runtime blueprint type".to_string())
                })?,
            None => self.default_runtime_blueprint(),
        };
        match mutation {
            PipelineMutation::MutateTransformGraph { mutation } => {
                blueprint
                    .transform_graph
                    .apply_mutation(mutation)
                    .map_err(|error| PipelineError::StageFailure(error.to_string()))?;
                blueprint
                    .transform_graph
                    .validate_unique_stage_keys()
                    .map_err(|error| PipelineError::StageFailure(error.to_string()))?;
            },
            PipelineMutation::SetMixerPlan { mixer } => {
                blueprint.mixer = mixer;
            },
            PipelineMutation::SetResamplerPlan { resampler } => {
                blueprint.resampler = resampler;
            },
            PipelineMutation::SetBuiltinTransformSlot { slot, enabled } => match slot {
                BuiltinTransformSlot::GaplessTrim => blueprint.builtin_slots.gapless_trim = enabled,
                BuiltinTransformSlot::TransitionGain => {
                    blueprint.builtin_slots.transition_gain = enabled
                },
                BuiltinTransformSlot::MasterGain => blueprint.builtin_slots.master_gain = enabled,
            },
            PipelineMutation::SetSinkRoute { route } => {
                blueprint.sink_route = route;
            },
        }
        Ok(Arc::new(blueprint))
    }

    fn create_runtime(&self) -> Box<dyn PipelineRuntime> {
        Box::new(BackendRuntime)
    }
}

struct BackendRuntime;

impl BackendAssembler {
    fn default_runtime_blueprint(&self) -> RuntimeBlueprint {
        let sink_route = current_sink_route_plan();
        let (output, plugin_prefers_track_rate) = current_blueprint_output_spec().unwrap_or((
            stellatune_audio_builtin_adapters::device_sink::OutputDeviceSpec {
                sample_rate: self.fallback_output_sample_rate.max(1),
                channels: self.fallback_output_channels.max(1),
            },
            None,
        ));
        let output_options = super::engine::snapshot_runtime_output_options();
        let resampler =
            if plugin_prefers_track_rate.unwrap_or(output_options.match_track_sample_rate) {
                None
            } else {
                Some(ResamplerPlan::new(
                    output.sample_rate,
                    output_options.resample_quality,
                ))
            };
        RuntimeBlueprint {
            transform_graph: TransformGraph::default(),
            mixer: Some(MixerPlan::new(output.channels, LfeMode::Mute)),
            resampler,
            builtin_slots: BuiltinTransformSlots::default(),
            sink_route,
        }
    }
}

impl PipelineRuntime for BackendRuntime {
    fn assemble(
        &mut self,
        blueprint: &dyn PipelineBlueprint,
    ) -> Result<AssembledPipeline, PipelineError> {
        let Some(blueprint) = (blueprint as &dyn Any).downcast_ref::<RuntimeBlueprint>() else {
            return Err(PipelineError::StageFailure(
                "unexpected runtime blueprint type".to_string(),
            ));
        };
        let plugin_stages = build_plugin_transform_stage_set_from_graph(&blueprint.transform_graph)
            .map_err(PipelineError::StageFailure)?;

        let decode = AssembledDecodePipeline {
            source: build_plugin_source(),
            decoder: Box::new(HybridDecoderStage::new()),
            transforms: plugin_stages.main,
            transform_chain: TransformChain {
                pre_mix: plugin_stages.pre_mix,
                post_mix: plugin_stages.post_mix,
            },
            mixer: blueprint.mixer,
            resampler: blueprint.resampler,
            builtin_slots: blueprint.builtin_slots,
        };
        let control = shared_device_sink_control();
        let (sink_stage, sink_route_fingerprint): (Box<dyn SinkStage>, u64) =
            match &blueprint.sink_route {
                PipelineSinkRoute::Plugin {
                    plugin_id,
                    type_id,
                    config_json,
                    target_json,
                } => {
                    let route = PluginOutputSinkRouteSpec::new(
                        plugin_id.clone(),
                        type_id.clone(),
                        config_json.clone(),
                        target_json.clone(),
                    )
                    .map_err(PipelineError::StageFailure)?;
                    let route_fingerprint = fingerprint_plugin_output_route(&route);
                    (
                        Box::new(PluginOutputSinkStage::new(route)),
                        route_fingerprint,
                    )
                },
                PipelineSinkRoute::Builtin { backend, device_id } => {
                    let backend = map_pipeline_output_backend(*backend);
                    let route_fingerprint =
                        fingerprint_builtin_output_route(backend, device_id.as_deref());
                    let stage: Box<dyn SinkStage> = match backend {
                        OutputBackend::Shared => Box::new(DeviceSinkStage::with_control(control)),
                        OutputBackend::WasapiExclusive => {
                            Box::new(WasapiExclusiveSinkStage::with_device_sink_control(control))
                        },
                    };
                    (stage, route_fingerprint)
                },
            };
        Ok(AssembledPipeline::from_parts(
            decode,
            Box::new(StaticSinkPlan::with_route_fingerprint(
                vec![sink_stage],
                sink_route_fingerprint,
            )),
        ))
    }
}

fn current_sink_route_plan() -> PipelineSinkRoute {
    let route_control = shared_runtime_sink_route_control();
    if let Some(route) = route_control.current_plugin_route() {
        return PipelineSinkRoute::Plugin {
            plugin_id: route.plugin_id,
            type_id: route.type_id,
            config_json: route.config_json,
            target_json: route.target_json,
        };
    }
    let control = shared_device_sink_control();
    let (backend, device_id) = control.desired_route();
    PipelineSinkRoute::Builtin {
        backend: map_output_backend_to_pipeline(backend),
        device_id,
    }
}

fn map_output_backend_to_pipeline(backend: OutputBackend) -> PipelineOutputBackend {
    match backend {
        OutputBackend::Shared => PipelineOutputBackend::Shared,
        OutputBackend::WasapiExclusive => PipelineOutputBackend::WasapiExclusive,
    }
}

fn map_pipeline_output_backend(backend: PipelineOutputBackend) -> OutputBackend {
    match backend {
        PipelineOutputBackend::Shared => OutputBackend::Shared,
        PipelineOutputBackend::WasapiExclusive => OutputBackend::WasapiExclusive,
    }
}
