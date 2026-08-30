use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use stellatune_audio::config::engine::LfeMode;
use stellatune_audio::pipeline::assembly::{
    AssembledPipeline, BuiltinTransformSlots, MixerPlan, PipelineOutputBackend, ResamplerPlan,
    SinkPlan, StaticSinkPlan,
};
use stellatune_audio::pipeline::capability::{
    CapabilityDescriptor, CapabilityKind, CapabilityRegistry, DecoderFactory, ExecutionBackend,
    OutputFactory, SourceFactory,
};
use stellatune_audio::pipeline::plan::{
    MediaHints, OutputSelection, PipelineBuilder, PipelinePlanner, PlaybackPolicies,
    PlaybackRequest, SourceCapabilities, SourceLocator, SourcePlan, SourceRequirements,
    StageConfig, StageId,
};
use stellatune_audio_builtin_adapters::device_sink::{
    DeviceSinkControl, DeviceSinkStage, OutputBackend,
};
use stellatune_audio_builtin_adapters::source_local::build_local_source;
use stellatune_audio_builtin_adapters::wasapi_exclusive_sink::WasapiExclusiveSinkStage;
use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::sink::SinkStage;
use stellatune_audio_core::pipeline::stages::{decoder::DecoderStage, source::SourceStage};

use super::engine::current_blueprint_output_spec;
use super::hybrid_decoder_stage::HybridDecoderStage;

const FALLBACK_OUTPUT_SAMPLE_RATE: u32 = 48_000;
const FALLBACK_OUTPUT_CHANNELS: u16 = 2;
const FILE_SOURCE_ID: &str = "builtin.native-source";
const HTTP_SOURCE_ID: &str = "builtin.native-http-source";
const HYBRID_DECODER_ID: &str = "builtin.native-decoder";
const RUNTIME_OUTPUT_ID: &str = "builtin.runtime-output";

fn shared_sink_control_cell() -> &'static OnceLock<DeviceSinkControl> {
    static CONTROL: OnceLock<DeviceSinkControl> = OnceLock::new();
    &CONTROL
}

pub fn shared_device_sink_control() -> DeviceSinkControl {
    shared_sink_control_cell()
        .get_or_init(DeviceSinkControl::default)
        .clone()
}

#[derive(Debug, Clone, Default)]
struct RuntimePipelineConfig {
    builtin_slots: BuiltinTransformSlots,
}

fn runtime_pipeline_config() -> &'static Mutex<RuntimePipelineConfig> {
    static CONFIG: OnceLock<Mutex<RuntimePipelineConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| Mutex::new(RuntimePipelineConfig::default()))
}

pub fn set_runtime_builtin_transform_options(gapless: bool, transition_gain: bool) {
    let mut config = runtime_pipeline_config()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    config.builtin_slots.gapless_trim = gapless;
    config.builtin_slots.transition_gain = transition_gain;
}

#[derive(Clone)]
pub struct BackendAssembler {
    fallback_output_sample_rate: u32,
    fallback_output_channels: u16,
    registry: Arc<CapabilityRegistry>,
}

impl Default for BackendAssembler {
    fn default() -> Self {
        Self {
            fallback_output_sample_rate: FALLBACK_OUTPUT_SAMPLE_RATE,
            fallback_output_channels: FALLBACK_OUTPUT_CHANNELS,
            registry: Arc::new(build_capability_registry()),
        }
    }
}

impl stellatune_audio::pipeline::assembly::PipelineFactory for BackendAssembler {
    fn build_pipeline(&self, input: &InputRef) -> Result<AssembledPipeline, PipelineError> {
        let InputRef::TrackToken(track_token) = input;
        if track_token.trim().is_empty() {
            return Err(PipelineError::StageFailure(
                "track token must not be empty".into(),
            ));
        }
        let slots = runtime_pipeline_config()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .builtin_slots;
        let (output, _) = current_blueprint_output_spec().unwrap_or((
            stellatune_audio_builtin_adapters::device_sink::OutputDeviceSpec {
                sample_rate: self.fallback_output_sample_rate,
                channels: self.fallback_output_channels,
            },
            None,
        ));
        let options = super::engine::snapshot_runtime_output_options();
        let request = PlaybackRequest {
            input: input.clone(),
            transforms: Vec::new(),
            output: OutputSelection {
                stage_id: stage_id(RUNTIME_OUTPUT_ID),
                config: StageConfig::validated(serde_json::json!({
                    "backend": match current_output_backend() {
                        PipelineOutputBackend::Shared => "shared",
                        PipelineOutputBackend::WasapiExclusive => "wasapi-exclusive",
                    }
                })),
            },
            policies: PlaybackPolicies {
                gapless: slots.gapless_trim,
                transition_gain: slots.transition_gain,
                master_gain: slots.master_gain,
                lfe_mode: LfeMode::Mute,
                resample_quality: options.resample_quality,
            },
            mixer: Some(MixerPlan::new(output.channels, LfeMode::Mute)),
            resampler: (!options.match_track_sample_rate)
                .then(|| ResamplerPlan::new(output.sample_rate, options.resample_quality)),
        };
        let source_plan = source_plan_for_track_token(track_token);
        let plan = PipelinePlanner::new(stage_id(FILE_SOURCE_ID), stage_id(HTTP_SOURCE_ID))
            .plan(request, source_plan, &self.registry)
            .map_err(|error| PipelineError::StageFailure(error.to_string()))?;
        PipelineBuilder::build(&plan, &self.registry)
    }
}

fn build_capability_registry() -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::new();
    for id in [FILE_SOURCE_ID, HTTP_SOURCE_ID] {
        registry
            .register_source(
                CapabilityDescriptor::new(
                    stage_id(id),
                    CapabilityKind::Source,
                    ExecutionBackend::BuiltinRust,
                ),
                Arc::new(create_native_source) as Arc<dyn SourceFactory>,
            )
            .unwrap_or_else(|error| panic!("register {id}: {error}"));
    }
    registry
        .register_decoder(
            CapabilityDescriptor::new(
                stage_id(HYBRID_DECODER_ID),
                CapabilityKind::Decoder,
                ExecutionBackend::BuiltinRust,
            )
            .with_priority(100)
            .with_extensions(["*".to_string()]),
            Arc::new(create_native_decoder) as Arc<dyn DecoderFactory>,
        )
        .unwrap_or_else(|error| panic!("register decoder: {error}"));
    registry
        .register_output(
            CapabilityDescriptor::new(
                stage_id(RUNTIME_OUTPUT_ID),
                CapabilityKind::Sink,
                ExecutionBackend::BuiltinRust,
            ),
            Arc::new(create_output) as Arc<dyn OutputFactory>,
        )
        .unwrap_or_else(|error| panic!("register output: {error}"));
    registry
}

fn stage_id(value: &str) -> StageId {
    StageId::new(value).unwrap_or_else(|error| panic!("invalid stage id {value}: {error}"))
}

fn create_native_source(_config: &StageConfig) -> Result<Box<dyn SourceStage>, PipelineError> {
    Ok(build_local_source())
}

fn create_native_decoder(_config: &StageConfig) -> Result<Box<dyn DecoderStage>, PipelineError> {
    Ok(Box::new(HybridDecoderStage::new()))
}

fn create_output(config: &StageConfig) -> Result<Box<dyn SinkPlan>, PipelineError> {
    let backend = match config
        .value()
        .get("backend")
        .and_then(serde_json::Value::as_str)
    {
        Some("shared") => OutputBackend::Shared,
        Some("wasapi-exclusive") => OutputBackend::WasapiExclusive,
        _ => {
            return Err(PipelineError::StageFailure(
                "unsupported output backend".into(),
            ));
        },
    };
    let control = shared_device_sink_control();
    let (_, device_id) = control.desired_route();
    let stage: Box<dyn SinkStage> = match backend {
        OutputBackend::Shared => Box::new(DeviceSinkStage::with_control(control)),
        OutputBackend::WasapiExclusive => {
            Box::new(WasapiExclusiveSinkStage::with_device_sink_control(control))
        },
    };
    let mut hasher = DefaultHasher::new();
    match backend {
        OutputBackend::Shared => 0_u8.hash(&mut hasher),
        OutputBackend::WasapiExclusive => 1_u8.hash(&mut hasher),
    }
    device_id.hash(&mut hasher);
    Ok(Box::new(StaticSinkPlan::with_route_fingerprint(
        vec![stage],
        hasher.finish(),
    )))
}

fn current_output_backend() -> PipelineOutputBackend {
    match shared_device_sink_control().desired_route().0 {
        OutputBackend::Shared => PipelineOutputBackend::Shared,
        OutputBackend::WasapiExclusive => PipelineOutputBackend::WasapiExclusive,
    }
}

fn source_plan_for_track_token(track_token: &str) -> SourcePlan {
    let locator = serde_json::from_str::<serde_json::Value>(track_token.trim())
        .ok()
        .and_then(|value| {
            value
                .get("locator")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| track_token.trim().to_string());
    let extension = locator
        .split(['?', '#'])
        .next()
        .and_then(|path| std::path::Path::new(path).extension())
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    let source_locator = if locator.starts_with("http://") || locator.starts_with("https://") {
        SourceLocator::Http {
            url: locator,
            headers: Default::default(),
        }
    } else {
        SourceLocator::File { path: locator }
    };
    SourcePlan {
        locator: source_locator,
        media: MediaHints {
            extension,
            mime_type: None,
            content_length: None,
        },
        capabilities: SourceCapabilities {
            seekable: true,
            live: false,
        },
        requirements: SourceRequirements::default(),
    }
}
