//! Shared external-track registration for native and plugin clients.
use super::{
    error::PlayerServiceError,
    identity::{ProviderId, ProviderTrackIdentityInput, ProviderTrackKeyInput, TrackId},
    service::PlayerService,
    source::SourceResolverSpec,
};
use crate::runtime::TypeScriptSourceResolver;
use std::sync::Arc;
use stellatune_plugins::typescript::{TypeScriptRuntime, manifest::TypeScriptCapabilityKind};

pub async fn ensure_provider_track(
    service: &PlayerService,
    runtime: Arc<TypeScriptRuntime>,
    plugin_id: &str,
    capability_id: &str,
    provider_id: &str,
    provider_key: &str,
) -> Result<TrackId, PlayerServiceError> {
    let plugin_id = plugin_id.trim();
    let capability_id = capability_id.trim();
    let registrations = runtime.registered_plugins().await;
    let capability = registrations
        .iter()
        .find(|p| p.manifest.id == plugin_id)
        .and_then(|p| {
            p.manifest
                .capabilities
                .iter()
                .find(|c| c.id == capability_id)
        })
        .ok_or_else(|| {
            PlayerServiceError::PluginCapabilityNotFound(format!("{plugin_id}::{capability_id}"))
        })?;
    if capability.kind != TypeScriptCapabilityKind::SourceResolver {
        return Err(PlayerServiceError::InvalidSourceSpec(
            "capability must be a source-resolver".into(),
        ));
    }
    ProviderId::new(provider_id.trim())?;
    let provider = ProviderId::new(format!(
        "{plugin_id}::{capability_id}::{}",
        provider_id.trim()
    ))?;
    let spec = SourceResolverSpec::new(plugin_id, capability_id, "{}")?;
    let resolver = Arc::new(TypeScriptSourceResolver::new(
        runtime,
        plugin_id,
        capability_id,
    ));
    let source = service
        .ensure_plugin_source(provider, spec, resolver)
        .await?;
    let key = provider_key.trim();
    let key = key
        .parse::<u64>()
        .ok()
        .filter(|v| v.to_string() == key)
        .map(ProviderTrackKeyInput::Numeric)
        .unwrap_or_else(|| ProviderTrackKeyInput::Text(key.into()));
    service
        .ensure_track(ProviderTrackIdentityInput {
            source_instance_id: source.get(),
            provider_key: key,
        })
        .await
}
