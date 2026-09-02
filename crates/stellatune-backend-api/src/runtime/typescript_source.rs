use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Map, Value};
use stellatune_plugins::typescript::TypeScriptRuntime;
use stellatune_plugins::typescript::protocol::{SourceLocatorDto, SourcePlanDto};

use crate::player_service::{
    MediaHintsInput, PlayerServiceError, ProviderTrackKey, ResolvedSourceSpec, SourceCatalogEntry,
    SourceResolutionInput, SourceResolver, SourceResolverFactory, SourceResolverSpec,
};

pub struct TypeScriptSourceResolverFactory {
    runtime: Arc<TypeScriptRuntime>,
}

impl TypeScriptSourceResolverFactory {
    pub fn new(runtime: Arc<TypeScriptRuntime>) -> Self {
        Self { runtime }
    }
}

impl SourceResolverFactory for TypeScriptSourceResolverFactory {
    fn create(
        &self,
        spec: &SourceResolverSpec,
    ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError> {
        let config = serde_json::from_str(&spec.config_json)
            .map_err(|error| PlayerServiceError::InvalidSourceSpec(error.to_string()))?;
        Ok(Arc::new(TypeScriptSourceResolver::new(
            Arc::clone(&self.runtime),
            spec.plugin_id.clone(),
            spec.capability_id.clone(),
            config,
        )?))
    }
}

pub struct TypeScriptSourceResolver {
    runtime: Arc<TypeScriptRuntime>,
    plugin_id: String,
    capability_id: String,
    config: Value,
}

impl TypeScriptSourceResolver {
    pub fn new(
        runtime: Arc<TypeScriptRuntime>,
        plugin_id: impl Into<String>,
        capability_id: impl Into<String>,
        config: Value,
    ) -> Result<Self, PlayerServiceError> {
        if !config.is_object() {
            return Err(PlayerServiceError::InvalidSourceSpec(
                "source resolver config must be an object".to_owned(),
            ));
        }
        Ok(Self {
            runtime,
            plugin_id: plugin_id.into(),
            capability_id: capability_id.into(),
            config,
        })
    }
}

#[async_trait]
impl SourceResolver for TypeScriptSourceResolver {
    async fn resolve(
        &self,
        _source: &SourceCatalogEntry,
        key: &ProviderTrackKey,
    ) -> Result<ResolvedSourceSpec, PlayerServiceError> {
        let mut input = Map::new();
        match key {
            ProviderTrackKey::Numeric(value) => {
                input.insert("song_id".to_owned(), Value::from(*value));
                input.insert("track_id".to_owned(), Value::from(*value));
                input.insert("track".to_owned(), serde_json::json!({ "song_id": value }));
            },
            ProviderTrackKey::Text(value) => {
                input.insert("track_id".to_owned(), Value::String(value.clone()));
                input.insert("track".to_owned(), serde_json::json!({ "track_id": value }));
            },
        }
        input.insert("config".to_owned(), self.config.clone());
        let invocation = self
            .runtime
            .invoke(
                &self.plugin_id,
                &self.capability_id,
                None,
                "resolve",
                Value::Object(input),
                None,
            )
            .await
            .map_err(|error| PlayerServiceError::Resolve(error.to_string()))?;
        let result: SourcePlanDto = serde_json::from_value(invocation.value)
            .map_err(|error| PlayerServiceError::InvalidSourceSpec(error.to_string()))?;
        let media = MediaHintsInput {
            extension: result.media.codec_hint,
            mime_type: result.media.mime_type,
            content_length: None,
            container_hint: None,
        };
        let input = match result.source {
            SourceLocatorDto::File { path } => SourceResolutionInput::File { path, media },
            SourceLocatorDto::Http { url, headers } => SourceResolutionInput::Http {
                url,
                headers,
                media,
                seekable: result.capabilities.seekable,
                live: result.capabilities.live,
            },
        };
        input.try_into()
    }
}
