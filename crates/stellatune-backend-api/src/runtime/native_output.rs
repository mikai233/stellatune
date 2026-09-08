//! Plugin discovery and output routing. HTTP/FFI do not own driver processes.
use super::{pipeline::shared_device_sink_control, shared_typescript_runtime};
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
use stellatune_audio_asio_adapter::{AsioConfig, AsioSinkFactory};
use stellatune_audio_builtin_adapters::factories::RuntimeDeviceSinkFactory;
use stellatune_audio_core::{
    error::FactoryError,
    format::PcmFormat,
    sink::{OutputCompatibilityKey, SinkFactory, SinkStage},
    stage::StageId,
};
use stellatune_plugins::typescript::manifest::TypeScriptCapabilityKind;

pub struct NativeOutputDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}
pub async fn native_output_types() -> Vec<NativeOutputDescriptor> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let mut result = Vec::new();
    for plugin in shared_typescript_runtime().registered_plugins().await {
        for capability in &plugin.manifest.capabilities {
            if capability.kind != TypeScriptCapabilityKind::OutputSink {
                continue;
            }
            let schema = if let Some(path) = &capability.config_schema {
                tokio::fs::read_to_string(plugin.package_root.join(path))
                    .await
                    .unwrap_or_else(|_| "{}".into())
            } else {
                "{}".into()
            };
            result.push(NativeOutputDescriptor {
                plugin_id: plugin.manifest.id.clone(),
                plugin_name: plugin.manifest.name.clone(),
                type_id: capability.id.clone(),
                display_name: capability.display_name.clone(),
                config_schema_json: schema,
                default_config_json: "{}".into(),
            });
        }
    }
    result
}
async fn executable(plugin_id: &str, type_id: &str) -> Result<PathBuf, String> {
    if !cfg!(windows) {
        return Err("ASIO output is only available on Windows".into());
    }
    let plugins = shared_typescript_runtime().registered_plugins().await;
    let plugin = plugins
        .iter()
        .find(|p| p.manifest.id == plugin_id)
        .ok_or_else(|| format!("output plugin is disabled or not installed: {plugin_id}"))?;
    let capability = plugin
        .manifest
        .capabilities
        .iter()
        .find(|c| c.id == type_id)
        .ok_or_else(|| format!("output capability does not exist: {type_id}"))?;
    if capability.kind != TypeScriptCapabilityKind::OutputSink {
        return Err("capability is not an output sink".into());
    }
    let output = capability
        .native_output
        .as_ref()
        .ok_or("native output host is missing")?;
    if output.protocol != "asio-v10" {
        return Err("unsupported native output protocol".into());
    }
    Ok(plugin.package_root.join(&output.executable))
}
pub async fn native_output_targets(
    plugin_id: String,
    type_id: String,
    config_json: String,
) -> Result<String, String> {
    let _guard = mutation_guard().await;
    let config: AsioConfig = serde_json::from_str(&config_json).map_err(|e| e.to_string())?;
    config.validate()?;
    let executable = executable(&plugin_id, &type_id).await?;
    tokio::task::spawn_blocking(move || {
        let devices = stellatune_audio_asio_adapter::list_devices(&executable)?;
        // Persist stable IDs only. A fresh host resolves its own selection token.
        let values: Vec<_> = devices
            .into_iter()
            .map(|d| serde_json::json!({"id": d.id, "name": d.name}))
            .collect();
        serde_json::to_string(&values).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

pub(super) struct Route {
    plugin_id: String,
    factory: Arc<AsioSinkFactory>,
}
pub(super) async fn suspend_route() -> Option<Route> {
    let previous = route()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(old) = &previous {
        let factory = Arc::clone(&old.factory);
        let _ = tokio::task::spawn_blocking(move || factory.release_device()).await;
    }
    previous
}
pub(super) fn restore_route(previous: Option<Route>) {
    *route()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = previous;
}
pub(super) fn retire_route(previous: Option<Route>) {
    if let Some(previous) = previous {
        previous.factory.revoke();
    }
}
fn route() -> &'static Mutex<Option<Route>> {
    static ROUTE: OnceLock<Mutex<Option<Route>>> = OnceLock::new();
    ROUTE.get_or_init(|| Mutex::new(None))
}
// Serialize output selection with package mutation, including asynchronous
// discovery: no stale selection may resurrect an uninstalled host.
pub(super) async fn mutation_guard() -> tokio::sync::MutexGuard<'static, ()> {
    static MUTATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    MUTATION.lock().await
}
fn active() -> Option<Arc<AsioSinkFactory>> {
    route()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|r| Arc::clone(&r.factory))
}
pub(super) fn active_format() -> Option<PcmFormat> {
    active().map(|f| f.format())
}

#[derive(Deserialize)]
struct Target {
    id: String,
}
pub(super) async fn set_route(
    plugin_id: String,
    type_id: String,
    config_json: String,
    target_json: String,
) -> Result<(), String> {
    let _guard = mutation_guard().await;
    let executable = executable(&plugin_id, &type_id).await?;
    let config: AsioConfig = serde_json::from_str(&config_json).map_err(|e| e.to_string())?;
    config.validate()?;
    let target: Target = serde_json::from_str(&target_json).map_err(|e| e.to_string())?;
    static REVISION: AtomicU64 = AtomicU64::new(1);
    let revision = REVISION.fetch_add(1, Ordering::Relaxed);
    let player = super::shared_playback_controller();
    let resume = player.snapshot().await.map_err(|e| e.to_string())?.state
        == stellatune_audio::playback::event::PlaybackState::Playing;
    if resume {
        player.pause().await.map_err(|e| e.to_string())?;
    }
    // Stop the old driver before probing: many ASIO drivers allow one client.
    let previous = suspend_route().await;
    let factory = tokio::task::spawn_blocking(move || {
        AsioSinkFactory::discover(executable, target.id, config, revision)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    let result = match factory {
        Ok(factory) => {
            *route()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(Route { plugin_id, factory });
            super::shared_playback_controller()
                .rebuild_output()
                .await
                .map_err(|e| e.to_string())
        },
        Err(error) => Err(error),
    };
    if result.is_err() {
        remove_route(None).await;
        restore_route(previous);
        let _ = super::shared_playback_controller().rebuild_output().await;
    } else {
        retire_route(previous);
    }
    if resume {
        let resumed = player.play().await.map_err(|e| e.to_string());
        result.and(resumed)
    } else {
        result
    }
}
/// Removes matching routes before touching package files. Caller owns mutation_guard.
pub(super) async fn remove_route(plugin_id: Option<&str>) -> bool {
    let old = {
        let mut slot = route()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot
            .as_ref()
            .is_some_and(|r| plugin_id.is_none_or(|id| id == r.plugin_id))
        {
            slot.take()
        } else {
            None
        }
    };
    if let Some(old) = old {
        let _ = tokio::task::spawn_blocking(move || old.factory.revoke()).await;
        true
    } else {
        false
    }
}
pub(super) async fn clear_route() -> Result<(), String> {
    let _guard = mutation_guard().await;
    if active().is_none() {
        return Ok(());
    }
    let player = super::shared_playback_controller();
    let resume = player.snapshot().await.map_err(|e| e.to_string())?.state
        == stellatune_audio::playback::event::PlaybackState::Playing;
    if resume {
        player.pause().await.map_err(|e| e.to_string())?;
    }
    if remove_route(None).await {
        let rebuilt = player.rebuild_output().await.map_err(|e| e.to_string());
        if resume {
            return rebuilt.and(player.play().await.map_err(|e| e.to_string()));
        }
        return rebuilt;
    }
    Ok(())
}

pub(super) struct RuntimeOutputFactory {
    builtin: RuntimeDeviceSinkFactory,
}
impl RuntimeOutputFactory {
    pub fn new() -> Self {
        Self {
            builtin: RuntimeDeviceSinkFactory::new(shared_device_sink_control(), 1),
        }
    }
    fn native_factory(&self) -> Option<AsioSinkFactory> {
        let options = *super::engine::runtime_output_options()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        active()
            .map(|factory| factory.with_match_track_sample_rate(options.match_track_sample_rate))
    }
}
impl SinkFactory for RuntimeOutputFactory {
    fn id(&self) -> &StageId {
        self.builtin.id()
    }
    fn preferred_format(&self, input: PcmFormat) -> Result<PcmFormat, FactoryError> {
        match self.native_factory() {
            Some(factory) => factory.preferred_format(input),
            None => self.builtin.preferred_format(input),
        }
    }
    fn compatibility_key(&self, format: PcmFormat) -> Result<OutputCompatibilityKey, FactoryError> {
        match self.native_factory() {
            Some(factory) => factory.compatibility_key(format),
            None => self.builtin.compatibility_key(format),
        }
    }
    fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
        match self.native_factory() {
            Some(factory) => factory.create(),
            None => self.builtin.create(),
        }
    }
}
