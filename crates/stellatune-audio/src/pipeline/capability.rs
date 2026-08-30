//! Capability descriptors, typed stage factories, and deterministic registry selection.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use stellatune_audio_core::pipeline::error::PipelineError;
use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
use stellatune_audio_core::pipeline::stages::source::SourceStage;
use stellatune_audio_core::pipeline::stages::transform::TransformStage;
use thiserror::Error;

use crate::pipeline::assembly::SinkPlan;
use crate::pipeline::plan::{SourcePlan, StageConfig, StageId};

/// Where a capability implementation executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionBackend {
    /// Native Rust implementation linked into the core process.
    BuiltinRust,
    /// TypeScript control-plane provider hosted by the shared Node runner.
    TypeScriptProcess,
    /// Native implementation hosted in an external process.
    ExternalProcess,
}

/// Capability slot exposed to planning and validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    /// Negotiates an input into a declarative source plan.
    SourceResolver,
    /// Supplies synchronized lyrics or lyric search results.
    LyricsProvider,
    /// Supplies authentication flows and refreshed credentials.
    AuthProvider,
    /// Reads encoded bytes in the Rust data plane.
    Source,
    /// Decodes encoded media into PCM.
    Decoder,
    /// Processes PCM in the native pipeline.
    Transform,
    /// Consumes PCM through a local or external sink proxy.
    Sink,
    /// Controls a network playback target without carrying PCM through RPC.
    NetworkControl,
}

/// Scheduling and realtime constraints for a capability implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageExecutionClass {
    /// Initialization, negotiation, authentication, and configuration only.
    Control,
    /// Bounded asynchronous production outside the realtime PCM callback.
    Buffered,
    /// Real-time PCM work with strict non-blocking constraints.
    Realtime,
}

/// Immutable metadata used by the planner without constructing a stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDescriptor {
    /// Stable capability identifier.
    pub id: StageId,
    /// Owning plugin, or `None` for core/builtin capabilities.
    pub plugin_id: Option<String>,
    /// Slot this capability can fill.
    pub kind: CapabilityKind,
    /// Execution boundary used to reject invalid real-time combinations.
    pub backend: ExecutionBackend,
    /// Scheduling and realtime constraints.
    pub execution_class: StageExecutionClass,
    /// Human-readable capability name.
    pub display_name: String,
    /// Higher values are preferred when decoder candidates otherwise match.
    pub priority: i32,
    /// Normalized extensions supported by decoder candidates.
    pub extensions: Vec<String>,
}

impl CapabilityDescriptor {
    /// Creates a descriptor with deterministic normalized extension metadata.
    pub fn new(id: StageId, kind: CapabilityKind, backend: ExecutionBackend) -> Self {
        let execution_class = match kind {
            CapabilityKind::SourceResolver
            | CapabilityKind::LyricsProvider
            | CapabilityKind::AuthProvider
            | CapabilityKind::NetworkControl => StageExecutionClass::Control,
            CapabilityKind::Source => StageExecutionClass::Buffered,
            CapabilityKind::Decoder | CapabilityKind::Transform | CapabilityKind::Sink => {
                StageExecutionClass::Realtime
            },
        };
        let display_name = id.as_str().to_string();
        Self {
            id,
            plugin_id: None,
            kind,
            backend,
            execution_class,
            display_name,
            priority: 0,
            extensions: Vec::new(),
        }
    }

    /// Associates a capability with its owning plugin.
    pub fn with_plugin_id(mut self, plugin_id: impl Into<String>) -> Self {
        self.plugin_id = Some(plugin_id.into());
        self
    }

    /// Sets the human-readable name.
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    /// Overrides the inferred execution class.
    pub fn with_execution_class(mut self, execution_class: StageExecutionClass) -> Self {
        self.execution_class = execution_class;
        self
    }

    /// Sets decoder priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Sets normalized, sorted, and deduplicated extension metadata.
    pub fn with_extensions(mut self, extensions: impl IntoIterator<Item = String>) -> Self {
        self.extensions = extensions
            .into_iter()
            .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();
        self.extensions.sort();
        self.extensions.dedup();
        self
    }
}

/// Registry validation and lookup failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    /// A descriptor kind does not match its typed factory slot.
    #[error("capability '{id}' has kind {actual:?}, expected {expected:?}")]
    KindMismatch {
        /// Capability identifier.
        id: String,
        /// Expected typed slot.
        expected: CapabilityKind,
        /// Descriptor-provided slot.
        actual: CapabilityKind,
    },
    /// TypeScript cannot be inserted into the PCM data plane.
    #[error("TypeScript capability '{id}' cannot fill {kind:?}")]
    TypeScriptInDataPlane {
        /// Capability identifier.
        id: String,
        /// Invalid PCM-facing kind.
        kind: CapabilityKind,
    },
    /// A capability identifier is already registered.
    #[error("capability '{id}' is already registered")]
    Duplicate {
        /// Duplicate identifier.
        id: String,
    },
    /// A requested capability does not exist in the typed slot.
    #[error("{kind:?} capability '{id}' is not registered")]
    Missing {
        /// Requested kind.
        kind: CapabilityKind,
        /// Requested identifier.
        id: String,
    },
}

/// Typed source negotiation input passed to control-plane resolvers.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveSourceRequest {
    /// Resolver-specific logical media locator or track identity.
    pub input: serde_json::Value,
    /// Validated user/plugin configuration.
    pub config: StageConfig,
}

/// Structured source negotiation failure before data-plane construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("source resolver '{capability_id}' failed: {message}")]
pub struct SourceResolveError {
    /// Stable resolver capability identifier.
    pub capability_id: String,
    /// Backend-specific error details safe for diagnostics.
    pub message: String,
}

/// Async control-plane source resolver. It returns declarations, never media bytes.
pub trait SourceResolver: Send + Sync {
    /// Resolves logical media identity into a file/HTTP/external source plan.
    fn resolve<'a>(
        &'a self,
        request: &'a ResolveSourceRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SourcePlan, SourceResolveError>> + Send + 'a>>;
}

/// Factory for builtin or TypeScript source resolver providers.
pub trait SourceResolverFactory: Send + Sync {
    /// Creates one resolver proxy from validated plugin configuration.
    fn create(&self, config: &StageConfig) -> Result<Arc<dyn SourceResolver>, SourceResolveError>;
}

/// Typed request shared by low-frequency provider protocols. Provider traits
/// remain separate so callers cannot accidentally route auth to lyrics or
/// network-control slots.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRequest {
    pub operation: String,
    pub input: serde_json::Value,
    pub config: StageConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("provider '{capability_id}' failed: {message}")]
pub struct ProviderError {
    pub capability_id: String,
    pub message: String,
}

pub trait LyricsProvider: Send + Sync {
    fn invoke<'a>(
        &'a self,
        request: &'a ProviderRequest,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ProviderError>> + Send + 'a>>;
}

pub trait AuthProvider: Send + Sync {
    fn invoke<'a>(
        &'a self,
        request: &'a ProviderRequest,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ProviderError>> + Send + 'a>>;
}

pub trait NetworkControlProvider: Send + Sync {
    fn invoke<'a>(
        &'a self,
        request: &'a ProviderRequest,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ProviderError>> + Send + 'a>>;
}

pub trait LyricsProviderFactory: Send + Sync {
    fn create(&self, config: &StageConfig) -> Result<Arc<dyn LyricsProvider>, ProviderError>;
}

pub trait AuthProviderFactory: Send + Sync {
    fn create(&self, config: &StageConfig) -> Result<Arc<dyn AuthProvider>, ProviderError>;
}

pub trait NetworkControlProviderFactory: Send + Sync {
    fn create(
        &self,
        config: &StageConfig,
    ) -> Result<Arc<dyn NetworkControlProvider>, ProviderError>;
}

/// Factory for native source stages.
pub trait SourceFactory: Send + Sync {
    /// Creates one source stage from validated configuration.
    fn create(&self, config: &StageConfig) -> Result<Box<dyn SourceStage>, PipelineError>;
}

impl<F> SourceFactory for F
where
    F: Fn(&StageConfig) -> Result<Box<dyn SourceStage>, PipelineError> + Send + Sync,
{
    fn create(&self, config: &StageConfig) -> Result<Box<dyn SourceStage>, PipelineError> {
        self(config)
    }
}

/// Factory for native decoder stages.
pub trait DecoderFactory: Send + Sync {
    /// Creates one decoder stage from validated configuration.
    fn create(&self, config: &StageConfig) -> Result<Box<dyn DecoderStage>, PipelineError>;
}

impl<F> DecoderFactory for F
where
    F: Fn(&StageConfig) -> Result<Box<dyn DecoderStage>, PipelineError> + Send + Sync,
{
    fn create(&self, config: &StageConfig) -> Result<Box<dyn DecoderStage>, PipelineError> {
        self(config)
    }
}

/// Factory for native transform stages.
pub trait TransformFactory: Send + Sync {
    /// Creates one transform stage from validated configuration.
    fn create(&self, config: &StageConfig) -> Result<Box<dyn TransformStage>, PipelineError>;
}

impl<F> TransformFactory for F
where
    F: Fn(&StageConfig) -> Result<Box<dyn TransformStage>, PipelineError> + Send + Sync,
{
    fn create(&self, config: &StageConfig) -> Result<Box<dyn TransformStage>, PipelineError> {
        self(config)
    }
}

/// Factory for output sink plans.
pub trait OutputFactory: Send + Sync {
    /// Creates a sink plan from validated configuration.
    fn create(&self, config: &StageConfig) -> Result<Box<dyn SinkPlan>, PipelineError>;
}

impl<F> OutputFactory for F
where
    F: Fn(&StageConfig) -> Result<Box<dyn SinkPlan>, PipelineError> + Send + Sync,
{
    fn create(&self, config: &StageConfig) -> Result<Box<dyn SinkPlan>, PipelineError> {
        self(config)
    }
}

struct Registered<F: ?Sized> {
    descriptor: CapabilityDescriptor,
    factory: Arc<F>,
}

impl<F: ?Sized> Clone for Registered<F> {
    fn clone(&self) -> Self {
        Self {
            descriptor: self.descriptor.clone(),
            factory: Arc::clone(&self.factory),
        }
    }
}

/// Typed capability registry. It owns descriptors and factories, never active stages.
#[derive(Clone, Default)]
pub struct CapabilityRegistry {
    descriptors: BTreeMap<StageId, CapabilityDescriptor>,
    source_resolvers: BTreeMap<StageId, Registered<dyn SourceResolverFactory>>,
    lyrics_providers: BTreeMap<StageId, Registered<dyn LyricsProviderFactory>>,
    auth_providers: BTreeMap<StageId, Registered<dyn AuthProviderFactory>>,
    network_controls: BTreeMap<StageId, Registered<dyn NetworkControlProviderFactory>>,
    sources: BTreeMap<StageId, Registered<dyn SourceFactory>>,
    decoders: BTreeMap<StageId, Registered<dyn DecoderFactory>>,
    transforms: BTreeMap<StageId, Registered<dyn TransformFactory>>,
    outputs: BTreeMap<StageId, Registered<dyn OutputFactory>>,
}

impl CapabilityRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers descriptor-only control-plane capability metadata.
    pub fn register_descriptor(
        &mut self,
        descriptor: CapabilityDescriptor,
    ) -> Result<(), RegistryError> {
        self.validate_descriptor(&descriptor)?;
        self.reserve(&descriptor)?;
        self.descriptors.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    /// Registers a source factory.
    pub fn register_source(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn SourceFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::Source,
            |registry, id, registered| {
                registry.sources.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    /// Registers a control-plane source resolver factory.
    pub fn register_source_resolver(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn SourceResolverFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::SourceResolver,
            |registry, id, registered| {
                registry.source_resolvers.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    pub fn register_lyrics_provider(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn LyricsProviderFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::LyricsProvider,
            |registry, id, registered| {
                registry.lyrics_providers.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    pub fn register_auth_provider(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn AuthProviderFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::AuthProvider,
            |registry, id, registered| {
                registry.auth_providers.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    pub fn register_network_control(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn NetworkControlProviderFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::NetworkControl,
            |registry, id, registered| {
                registry.network_controls.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    /// Registers a decoder factory.
    pub fn register_decoder(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn DecoderFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::Decoder,
            |registry, id, registered| {
                registry.decoders.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    /// Registers a transform factory.
    pub fn register_transform(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn TransformFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::Transform,
            |registry, id, registered| {
                registry.transforms.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    /// Registers an output factory.
    pub fn register_output(
        &mut self,
        descriptor: CapabilityDescriptor,
        factory: Arc<dyn OutputFactory>,
    ) -> Result<(), RegistryError> {
        self.register_typed(
            descriptor,
            CapabilityKind::Sink,
            |registry, id, registered| {
                registry.outputs.insert(
                    id,
                    Registered {
                        descriptor: registered,
                        factory,
                    },
                );
            },
        )
    }

    /// Returns descriptor metadata by identifier.
    pub fn descriptor(&self, id: &StageId) -> Option<&CapabilityDescriptor> {
        self.descriptors.get(id)
    }

    /// Returns decoder descriptors ordered by score, then stable identifier.
    pub fn decoder_candidates(&self, extension: Option<&str>) -> Vec<&CapabilityDescriptor> {
        let extension = extension
            .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        let mut candidates: Vec<_> = self
            .decoders
            .values()
            .filter(|registered| {
                extension.as_ref().is_none_or(|extension| {
                    registered.descriptor.extensions.is_empty()
                        || registered
                            .descriptor
                            .extensions
                            .iter()
                            .any(|item| item == "*" || item == extension)
                })
            })
            .map(|registered| &registered.descriptor)
            .collect();
        candidates.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.id.cmp(&right.id))
        });
        candidates
    }

    pub(crate) fn source_factory(&self, id: &StageId) -> Result<&dyn SourceFactory, RegistryError> {
        self.sources
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::Source, id))
    }

    /// Returns a registered control-plane resolver factory.
    pub fn source_resolver_factory(
        &self,
        id: &StageId,
    ) -> Result<&dyn SourceResolverFactory, RegistryError> {
        self.source_resolvers
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::SourceResolver, id))
    }

    pub fn lyrics_provider_factory(
        &self,
        id: &StageId,
    ) -> Result<&dyn LyricsProviderFactory, RegistryError> {
        self.lyrics_providers
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::LyricsProvider, id))
    }

    pub fn auth_provider_factory(
        &self,
        id: &StageId,
    ) -> Result<&dyn AuthProviderFactory, RegistryError> {
        self.auth_providers
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::AuthProvider, id))
    }

    pub fn network_control_factory(
        &self,
        id: &StageId,
    ) -> Result<&dyn NetworkControlProviderFactory, RegistryError> {
        self.network_controls
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::NetworkControl, id))
    }

    pub(crate) fn decoder_factory(
        &self,
        id: &StageId,
    ) -> Result<&dyn DecoderFactory, RegistryError> {
        self.decoders
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::Decoder, id))
    }

    pub(crate) fn transform_factory(
        &self,
        id: &StageId,
    ) -> Result<&dyn TransformFactory, RegistryError> {
        self.transforms
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::Transform, id))
    }

    pub(crate) fn output_factory(&self, id: &StageId) -> Result<&dyn OutputFactory, RegistryError> {
        self.outputs
            .get(id)
            .map(|registered| registered.factory.as_ref())
            .ok_or_else(|| missing(CapabilityKind::Sink, id))
    }

    fn register_typed(
        &mut self,
        descriptor: CapabilityDescriptor,
        expected: CapabilityKind,
        insert: impl FnOnce(&mut Self, StageId, CapabilityDescriptor),
    ) -> Result<(), RegistryError> {
        if descriptor.kind != expected {
            return Err(RegistryError::KindMismatch {
                id: descriptor.id.as_str().to_string(),
                expected,
                actual: descriptor.kind,
            });
        }
        self.validate_descriptor(&descriptor)?;
        self.reserve(&descriptor)?;
        let id = descriptor.id.clone();
        self.descriptors.insert(id.clone(), descriptor.clone());
        insert(self, id, descriptor);
        Ok(())
    }

    fn validate_descriptor(&self, descriptor: &CapabilityDescriptor) -> Result<(), RegistryError> {
        if descriptor.backend == ExecutionBackend::TypeScriptProcess
            && matches!(
                descriptor.kind,
                CapabilityKind::Source
                    | CapabilityKind::Decoder
                    | CapabilityKind::Transform
                    | CapabilityKind::Sink
            )
        {
            return Err(RegistryError::TypeScriptInDataPlane {
                id: descriptor.id.as_str().to_string(),
                kind: descriptor.kind,
            });
        }
        Ok(())
    }

    fn reserve(&self, descriptor: &CapabilityDescriptor) -> Result<(), RegistryError> {
        if self.descriptors.contains_key(&descriptor.id) {
            return Err(RegistryError::Duplicate {
                id: descriptor.id.as_str().to_string(),
            });
        }
        Ok(())
    }
}

fn missing(kind: CapabilityKind, id: &StageId) -> RegistryError {
    RegistryError::Missing {
        kind,
        id: id.as_str().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stellatune_audio_core::pipeline::{error::PipelineError, stages::decoder::DecoderStage};

    use super::{
        CapabilityDescriptor, CapabilityKind, CapabilityRegistry, ExecutionBackend, RegistryError,
    };
    use crate::pipeline::plan::StageId;

    fn unused_decoder_factory(
        _: &crate::pipeline::plan::StageConfig,
    ) -> Result<Box<dyn DecoderStage>, PipelineError> {
        Err(PipelineError::StageFailure(
            "factory must not run in this test".to_string(),
        ))
    }

    #[test]
    fn typescript_is_rejected_from_pcm_stage_slots() {
        let mut registry = CapabilityRegistry::new();
        let descriptor = CapabilityDescriptor::new(
            StageId::new("ts.decoder").unwrap(),
            CapabilityKind::Decoder,
            ExecutionBackend::TypeScriptProcess,
        );
        let result = registry.register_decoder(
            descriptor,
            Arc::new(unused_decoder_factory) as Arc<dyn super::DecoderFactory>,
        );
        assert!(matches!(
            result,
            Err(RegistryError::TypeScriptInDataPlane { .. })
        ));
    }

    #[test]
    fn decoder_selection_is_priority_then_stable_id() {
        let mut registry = CapabilityRegistry::new();
        for (id, priority) in [("decoder.b", 10), ("decoder.a", 10), ("decoder.c", 5)] {
            let descriptor = CapabilityDescriptor::new(
                StageId::new(id).unwrap(),
                CapabilityKind::Decoder,
                ExecutionBackend::BuiltinRust,
            )
            .with_priority(priority)
            .with_extensions(["flac".to_string()]);
            registry
                .register_decoder(
                    descriptor,
                    Arc::new(unused_decoder_factory) as Arc<dyn super::DecoderFactory>,
                )
                .unwrap();
        }
        assert_eq!(
            registry
                .decoder_candidates(Some(".FLAC"))
                .into_iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["decoder.a", "decoder.b", "decoder.c"]
        );
    }
}
