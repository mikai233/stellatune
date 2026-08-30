use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use stellatune_audio::pipeline::capability::{
    AuthProvider, AuthProviderFactory, LyricsProvider, LyricsProviderFactory,
    NetworkControlProvider, NetworkControlProviderFactory, ProviderError, ProviderRequest,
};
use stellatune_audio::pipeline::plan::StageConfig;
use stellatune_plugins::typescript::TypeScriptRuntime;

#[derive(Clone)]
struct TypeScriptProviderProxy {
    runtime: Arc<TypeScriptRuntime>,
    plugin_id: String,
    capability_id: String,
}

impl TypeScriptProviderProxy {
    async fn invoke(&self, request: &ProviderRequest) -> Result<serde_json::Value, ProviderError> {
        let mut input = request.input.clone();
        if let Some(object) = input.as_object_mut() {
            object.insert("config".to_string(), request.config.value().clone());
        }
        self.runtime
            .invoke(
                &self.plugin_id,
                &self.capability_id,
                None,
                &request.operation,
                input,
                None,
            )
            .await
            .map(|result| result.value)
            .map_err(|error| ProviderError {
                capability_id: format!("{}::{}", self.plugin_id, self.capability_id),
                message: error.to_string(),
            })
    }
}

macro_rules! provider_proxy {
    ($proxy:ident, $factory:ident, $provider_trait:ident, $factory_trait:ident) => {
        pub struct $proxy(TypeScriptProviderProxy);

        impl $provider_trait for $proxy {
            fn invoke<'a>(
                &'a self,
                request: &'a ProviderRequest,
            ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ProviderError>> + Send + 'a>>
            {
                Box::pin(self.0.invoke(request))
            }
        }

        pub struct $factory {
            runtime: Arc<TypeScriptRuntime>,
            plugin_id: String,
            capability_id: String,
        }

        impl $factory {
            pub fn new(
                runtime: Arc<TypeScriptRuntime>,
                plugin_id: impl Into<String>,
                capability_id: impl Into<String>,
            ) -> Self {
                Self {
                    runtime,
                    plugin_id: plugin_id.into(),
                    capability_id: capability_id.into(),
                }
            }
        }

        impl $factory_trait for $factory {
            fn create(
                &self,
                _config: &StageConfig,
            ) -> Result<Arc<dyn $provider_trait>, ProviderError> {
                Ok(Arc::new($proxy(TypeScriptProviderProxy {
                    runtime: Arc::clone(&self.runtime),
                    plugin_id: self.plugin_id.clone(),
                    capability_id: self.capability_id.clone(),
                })))
            }
        }
    };
}

provider_proxy!(
    TypeScriptLyricsProviderProxy,
    TypeScriptLyricsProviderFactory,
    LyricsProvider,
    LyricsProviderFactory
);
provider_proxy!(
    TypeScriptAuthProviderProxy,
    TypeScriptAuthProviderFactory,
    AuthProvider,
    AuthProviderFactory
);
provider_proxy!(
    TypeScriptNetworkControlProxy,
    TypeScriptNetworkControlFactory,
    NetworkControlProvider,
    NetworkControlProviderFactory
);
