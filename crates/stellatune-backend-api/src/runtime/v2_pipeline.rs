use std::any::Any;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use stellatune_audio_builtin_adapters::device_sink::{
    DeviceSinkControl, DeviceSinkStage, OutputBackend, default_output_spec_for_backend,
    output_spec_for_route,
};
use stellatune_audio_builtin_adapters::wasapi_exclusive_sink::WasapiExclusiveSinkStage;
use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_plugin_adapters::decoder_stage::PluginDecoderStage;
use stellatune_audio_plugin_adapters::output_sink_stage::{
    PluginOutputSinkRouteSpec, PluginOutputSinkStage,
};
use stellatune_audio_plugin_adapters::source_plugin::build_plugin_source;
use stellatune_audio_plugin_adapters::transform_stage::build_plugin_transform_stage_set_from_graph;
use stellatune_audio_v2::assembly::{
    AssembledDecodePipeline, AssembledPipeline, BuiltinTransformSlot, BuiltinTransformSlots,
    MixerPlan, OpaqueTransformStageSpec, PipelineAssembler, PipelineMutation, PipelinePlan,
    PipelineRuntime, ResamplerPlan, StaticSinkPlan, TransformChain,
};
use stellatune_audio_v2::pipeline_graph::TransformGraph;
use stellatune_audio_v2::types::{LfeMode, ResampleQuality};

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
struct V2RuntimePlan {
    track_token: String,
}

#[derive(Debug, Clone)]
pub struct V2BackendAssembler {
    fallback_output_sample_rate: u32,
    fallback_output_channels: u16,
}

impl Default for V2BackendAssembler {
    fn default() -> Self {
        Self {
            fallback_output_sample_rate: FALLBACK_OUTPUT_SAMPLE_RATE,
            fallback_output_channels: FALLBACK_OUTPUT_CHANNELS,
        }
    }
}

impl PipelineAssembler for V2BackendAssembler {
    fn plan(&self, input: &InputRef) -> Result<Arc<dyn PipelinePlan>, PipelineError> {
        let InputRef::TrackToken(track_token) = input;
        if track_token.trim().is_empty() {
            return Err(PipelineError::StageFailure(
                "track token must not be empty".to_string(),
            ));
        }
        Ok(Arc::new(V2RuntimePlan {
            track_token: track_token.clone(),
        }))
    }

    fn create_runtime(&self) -> Box<dyn PipelineRuntime> {
        Box::new(V2BackendRuntime::new(
            self.fallback_output_sample_rate,
            self.fallback_output_channels,
        ))
    }
}

struct V2BackendRuntime {
    transform_graph: TransformGraph<OpaqueTransformStageSpec>,
    mixer: Option<MixerPlan>,
    resampler: Option<ResamplerPlan>,
    builtin_slots: BuiltinTransformSlots,
    fallback_output_sample_rate: u32,
    fallback_output_channels: u16,
}

impl V2BackendRuntime {
    fn new(fallback_output_sample_rate: u32, fallback_output_channels: u16) -> Self {
        let mut runtime = Self {
            transform_graph: TransformGraph::default(),
            mixer: None,
            resampler: None,
            builtin_slots: BuiltinTransformSlots::default(),
            fallback_output_sample_rate: fallback_output_sample_rate.max(1),
            fallback_output_channels: fallback_output_channels.max(1),
        };
        runtime.reset_output_plans();
        runtime
    }

    fn reset_output_plans(&mut self) {
        let control = shared_device_sink_control();
        let (backend, device_id) = control.desired_route();
        let output = output_spec_for_route(backend, device_id.as_deref())
            .or_else(|_| default_output_spec_for_backend(backend))
            .unwrap_or_else(
                |_| stellatune_audio_builtin_adapters::device_sink::OutputDeviceSpec {
                    sample_rate: self.fallback_output_sample_rate,
                    channels: self.fallback_output_channels,
                },
            );
        self.mixer = Some(MixerPlan::new(output.channels, LfeMode::Mute));
        self.resampler = Some(ResamplerPlan::new(
            output.sample_rate,
            ResampleQuality::High,
        ));
    }
}

impl PipelineRuntime for V2BackendRuntime {
    fn ensure(&mut self, plan: &dyn PipelinePlan) -> Result<AssembledPipeline, PipelineError> {
        let Some(plan) = (plan as &dyn Any).downcast_ref::<V2RuntimePlan>() else {
            return Err(PipelineError::StageFailure(
                "unexpected v2 runtime plan type".to_string(),
            ));
        };
        let plugin_stages = build_plugin_transform_stage_set_from_graph(&self.transform_graph)
            .map_err(PipelineError::StageFailure)?;

        let decode = AssembledDecodePipeline {
            source: build_plugin_source(plan.track_token.clone()),
            decoder: Box::new(PluginDecoderStage::new()),
            transforms: plugin_stages.main,
            transform_chain: TransformChain {
                pre_mix: plugin_stages.pre_mix,
                post_mix: plugin_stages.post_mix,
            },
            mixer: self.mixer,
            resampler: self.resampler,
            builtin_slots: self.builtin_slots,
        };
        let control = shared_device_sink_control();
        let route_control = shared_runtime_sink_route_control();
        let (sink_stage, sink_route_fingerprint): (Box<dyn SinkStage>, u64) =
            if let Some(plugin_route) = route_control.current_plugin_route() {
                let route_fingerprint = fingerprint_plugin_output_route(&plugin_route);
                (
                    Box::new(PluginOutputSinkStage::new(plugin_route)),
                    route_fingerprint,
                )
            } else {
                let (backend, device_id) = control.desired_route();
                let route_fingerprint =
                    fingerprint_builtin_output_route(backend, device_id.as_deref());
                let stage: Box<dyn SinkStage> = match backend {
                    OutputBackend::Shared => Box::new(DeviceSinkStage::with_control(control)),
                    OutputBackend::WasapiExclusive => {
                        Box::new(WasapiExclusiveSinkStage::with_device_sink_control(control))
                    },
                };
                (stage, route_fingerprint)
            };
        Ok(AssembledPipeline::from_parts(
            decode,
            Box::new(StaticSinkPlan::with_route_fingerprint(
                vec![sink_stage],
                sink_route_fingerprint,
            )),
        ))
    }

    fn apply_pipeline_mutation(&mut self, mutation: PipelineMutation) -> Result<(), PipelineError> {
        match mutation {
            PipelineMutation::MutateTransformGraph { mutation } => {
                self.transform_graph
                    .apply_mutation(mutation)
                    .map_err(PipelineError::StageFailure)?;
                self.transform_graph
                    .validate_unique_stage_keys()
                    .map_err(PipelineError::StageFailure)
            },
            PipelineMutation::SetMixerPlan { mixer } => {
                self.mixer = mixer;
                Ok(())
            },
            PipelineMutation::SetResamplerPlan { resampler } => {
                self.resampler = resampler;
                Ok(())
            },
            PipelineMutation::SetBuiltinTransformSlot { slot, enabled } => {
                match slot {
                    BuiltinTransformSlot::GaplessTrim => self.builtin_slots.gapless_trim = enabled,
                    BuiltinTransformSlot::TransitionGain => {
                        self.builtin_slots.transition_gain = enabled
                    },
                    BuiltinTransformSlot::MasterGain => self.builtin_slots.master_gain = enabled,
                }
                Ok(())
            },
        }
    }

    fn reset(&mut self) {
        self.transform_graph = TransformGraph::default();
        self.builtin_slots = BuiltinTransformSlots::default();
        self.reset_output_plans();
    }
}
