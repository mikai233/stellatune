use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;

use super::manifest::{TypeScriptPluginManifest, validate_typescript_manifest};
use super::process::{
    InvocationResult, PluginProcessConfig, PluginProcessHandle, PluginProcessSnapshot,
    PluginRuntimeError,
};

#[derive(Debug, Clone)]
pub struct RegisteredTypeScriptPlugin {
    pub manifest: TypeScriptPluginManifest,
    pub package_root: PathBuf,
}

struct RuntimeEntry {
    registration: RegisteredTypeScriptPlugin,
    process: Option<PluginProcessHandle>,
}

#[derive(Debug, Error)]
pub enum TypeScriptRuntimeError {
    #[error("TypeScript plugin '{0}' is not registered or is disabled")]
    NotRegistered(String),
    #[error(transparent)]
    Manifest(#[from] super::manifest::ManifestV2Error),
    #[error(transparent)]
    Process(#[from] PluginRuntimeError),
}

/// Registry of TypeScript control-plane plugins with at most one process per plugin.
#[derive(Clone)]
pub struct TypeScriptRuntime {
    inner: Arc<Mutex<BTreeMap<String, RuntimeEntry>>>,
    node_binary: PathBuf,
    runner_script: PathBuf,
    request_timeout: Duration,
    idle_timeout: Duration,
}

impl TypeScriptRuntime {
    pub fn new(runner_script: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BTreeMap::new())),
            node_binary: PathBuf::from("node"),
            runner_script: runner_script.into(),
            request_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
        }
    }

    pub fn with_node_binary(mut self, node_binary: impl Into<PathBuf>) -> Self {
        self.node_binary = node_binary.into();
        self
    }

    pub fn with_timeouts(mut self, request_timeout: Duration, idle_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self.idle_timeout = idle_timeout;
        self
    }

    pub async fn register(
        &self,
        manifest: TypeScriptPluginManifest,
        package_root: impl Into<PathBuf>,
    ) -> Result<(), TypeScriptRuntimeError> {
        let package_root = package_root.into();
        validate_typescript_manifest(&manifest, &package_root)?;
        let plugin_id = manifest.id.clone();
        let previous = self.inner.lock().await.remove(&plugin_id);
        if let Some(process) = previous.and_then(|entry| entry.process) {
            process.shutdown().await?;
        }
        self.inner.lock().await.insert(
            plugin_id,
            RuntimeEntry {
                registration: RegisteredTypeScriptPlugin {
                    manifest,
                    package_root,
                },
                process: None,
            },
        );
        Ok(())
    }

    pub async fn unregister(&self, plugin_id: &str) -> Result<(), TypeScriptRuntimeError> {
        let entry = self.inner.lock().await.remove(plugin_id);
        if let Some(process) = entry.and_then(|entry| entry.process) {
            process.shutdown().await?;
        }
        Ok(())
    }

    pub async fn invoke(
        &self,
        plugin_id: &str,
        capability_id: &str,
        instance_id: Option<String>,
        operation: &str,
        input: Value,
        expected_generation: Option<u64>,
    ) -> Result<InvocationResult, TypeScriptRuntimeError> {
        let process = {
            let mut entries = self.inner.lock().await;
            let entry = entries
                .get_mut(plugin_id)
                .ok_or_else(|| TypeScriptRuntimeError::NotRegistered(plugin_id.to_string()))?;
            if !entry
                .registration
                .manifest
                .capabilities
                .iter()
                .any(|capability| capability.id == capability_id)
            {
                return Err(TypeScriptRuntimeError::NotRegistered(format!(
                    "{plugin_id}::{capability_id}"
                )));
            }
            entry
                .process
                .get_or_insert_with(|| {
                    PluginProcessHandle::spawn(self.process_config(&entry.registration))
                })
                .clone()
        };
        Ok(process
            .invoke(
                capability_id,
                instance_id,
                operation,
                input,
                expected_generation,
            )
            .await?)
    }

    pub async fn stop_process(&self, plugin_id: &str) -> Result<(), TypeScriptRuntimeError> {
        let process = self
            .inner
            .lock()
            .await
            .get_mut(plugin_id)
            .and_then(|entry| entry.process.take());
        if let Some(process) = process {
            process.shutdown().await?;
        }
        Ok(())
    }

    pub async fn process_snapshot(
        &self,
        plugin_id: &str,
    ) -> Result<Option<PluginProcessSnapshot>, TypeScriptRuntimeError> {
        let process = self
            .inner
            .lock()
            .await
            .get(plugin_id)
            .and_then(|entry| entry.process.clone());
        match process {
            Some(process) => Ok(Some(process.snapshot().await?)),
            None => Ok(None),
        }
    }

    pub async fn registered_plugins(&self) -> Vec<RegisteredTypeScriptPlugin> {
        self.inner
            .lock()
            .await
            .values()
            .map(|entry| entry.registration.clone())
            .collect()
    }

    pub async fn shutdown(&self) -> Result<(), TypeScriptRuntimeError> {
        let processes = {
            let mut entries = self.inner.lock().await;
            entries
                .values_mut()
                .filter_map(|entry| entry.process.take())
                .collect::<Vec<_>>()
        };
        for process in processes {
            process.shutdown().await?;
        }
        Ok(())
    }

    fn process_config(&self, registration: &RegisteredTypeScriptPlugin) -> PluginProcessConfig {
        let mut config = PluginProcessConfig::new(
            registration.manifest.id.clone(),
            registration
                .package_root
                .join(&registration.manifest.runtime.entry),
            &self.runner_script,
        );
        config.node_binary.clone_from(&self.node_binary);
        config
            .protocol
            .clone_from(&registration.manifest.runtime.protocol);
        config.request_timeout = self.request_timeout;
        config.idle_timeout = self.idle_timeout;
        config
    }
}
