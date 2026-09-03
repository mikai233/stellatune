use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use lattice_actor::context::{ActorContext, HandlerContext};
use lattice_actor::error::{ActorError, ActorStopError};
use lattice_actor::handle::ActorHandle;
use lattice_actor::mailbox::MailboxConfig;
use lattice_actor::reply::ReplyTo;
use lattice_actor::runtime::spawn_actor;
use lattice_actor::state_machine::Stateless;
use lattice_actor::traits::{Actor, Responder, StopReason};
use thiserror::Error;

use stellatune_audio::playback::control::PlaybackController;
use stellatune_audio::playback::event::PlaybackState;
use stellatune_plugins::typescript::TypeScriptRuntime;
use stellatune_plugins::typescript::package::{
    InstalledTypeScriptPlugin, discover_typescript_plugins, install_typescript_artifact,
    uninstall_typescript_plugin,
};

#[derive(Debug, Error)]
#[error("plugin manager {operation} failed: {message}")]
pub struct PluginManagerOperationError {
    operation: &'static str,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledV2PluginSummary {
    pub id: String,
    pub version: String,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<Vec<InstalledV2PluginSummary>, PluginManagerOperationError>)]
struct DiscoverPlugins;

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<Vec<InstalledV2PluginSummary>, PluginManagerOperationError>)]
struct ReconcilePlugins {
    disabled_plugin_ids: HashSet<String>,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<InstalledV2PluginSummary, PluginManagerOperationError>)]
struct InstallPlugin {
    artifact_path: PathBuf,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<(), PluginManagerOperationError>)]
struct SetPluginEnabled {
    plugin_id: String,
    enabled: bool,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<(), PluginManagerOperationError>)]
struct UninstallPlugin {
    plugin_id: String,
}

struct PluginManagerActor {
    player: PlaybackController,
    resume_after_change: AtomicBool,
    runtime: Arc<TypeScriptRuntime>,
    plugins_dir: PathBuf,
}

impl Actor for PluginManagerActor {
    type Error = ActorError;
    type Behavior = Stateless;

    async fn stopping(
        &mut self,
        _ctx: &mut ActorContext<Self>,
        _reason: StopReason,
    ) -> Result<(), ActorStopError> {
        let _ = self.runtime.shutdown().await;
        Ok(())
    }
}

impl Responder<DiscoverPlugins> for PluginManagerActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: DiscoverPlugins,
        reply_to: ReplyTo<Result<Vec<InstalledV2PluginSummary>, PluginManagerOperationError>>,
    ) -> Result<(), ActorError> {
        let plugins_dir = self.plugins_dir.clone();
        let result = tokio::task::spawn_blocking(move || discover_typescript_plugins(&plugins_dir))
            .await
            .map_err(|error| manager_error("discover", error.to_string()))
            .and_then(|result| {
                result.map_err(|error| manager_error("discover", error.to_string()))
            });
        let result = match result {
            Ok(plugins) => {
                let mut summaries = Vec::with_capacity(plugins.len());
                for plugin in plugins {
                    if let Err(error) = self
                        .runtime
                        .register(plugin.manifest.clone(), &plugin.root_dir)
                        .await
                    {
                        let _ = reply_to.send(Err(manager_error("discover", error.to_string())));
                        return Ok(());
                    }
                    summaries.push(summary(&plugin));
                }
                Ok(summaries)
            },
            Err(error) => Err(error),
        };
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<InstallPlugin> for PluginManagerActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: InstallPlugin,
        reply_to: ReplyTo<Result<InstalledV2PluginSummary, PluginManagerOperationError>>,
    ) -> Result<(), ActorError> {
        let result = self.install(message.artifact_path).await;
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<ReconcilePlugins> for PluginManagerActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: ReconcilePlugins,
        reply_to: ReplyTo<Result<Vec<InstalledV2PluginSummary>, PluginManagerOperationError>>,
    ) -> Result<(), ActorError> {
        let result = self.reconcile(message.disabled_plugin_ids).await;
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<SetPluginEnabled> for PluginManagerActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: SetPluginEnabled,
        reply_to: ReplyTo<Result<(), PluginManagerOperationError>>,
    ) -> Result<(), ActorError> {
        let result = self.set_enabled(message.plugin_id, message.enabled).await;
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl Responder<UninstallPlugin> for PluginManagerActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: UninstallPlugin,
        reply_to: ReplyTo<Result<(), PluginManagerOperationError>>,
    ) -> Result<(), ActorError> {
        let result = self.uninstall(message.plugin_id).await;
        let _ = reply_to.send(result);
        Ok(())
    }
}

impl PluginManagerActor {
    async fn reconcile(
        &self,
        disabled_plugin_ids: HashSet<String>,
    ) -> Result<Vec<InstalledV2PluginSummary>, PluginManagerOperationError> {
        const OPERATION: &str = "reconcile";
        self.begin_change(OPERATION).await?;
        if let Err(error) = self.runtime.shutdown().await {
            self.abort_change().await;
            return Err(manager_error(OPERATION, error.to_string()));
        }
        for registration in self.runtime.registered_plugins().await {
            self.runtime
                .unregister(&registration.manifest.id)
                .await
                .map_err(|error| manager_error(OPERATION, error.to_string()))?;
        }
        let plugins_dir = self.plugins_dir.clone();
        let plugins =
            tokio::task::spawn_blocking(move || discover_typescript_plugins(&plugins_dir))
                .await
                .map_err(|error| manager_error(OPERATION, error.to_string()))
                .and_then(|result| {
                    result.map_err(|error| manager_error(OPERATION, error.to_string()))
                });
        let plugins = match plugins {
            Ok(plugins) => plugins,
            Err(error) => {
                self.abort_change().await;
                return Err(error);
            },
        };
        let mut summaries = Vec::new();
        for plugin in plugins {
            if disabled_plugin_ids.contains(&plugin.manifest.id) {
                self.runtime
                    .unregister(&plugin.manifest.id)
                    .await
                    .map_err(|error| manager_error(OPERATION, error.to_string()))?;
                continue;
            }
            self.runtime
                .register(plugin.manifest.clone(), &plugin.root_dir)
                .await
                .map_err(|error| manager_error(OPERATION, error.to_string()))?;
            summaries.push(summary(&plugin));
        }
        if let Err(error) = self.finish_change(OPERATION).await {
            self.abort_change().await;
            return Err(error);
        }
        Ok(summaries)
    }

    async fn begin_change(
        &self,
        operation: &'static str,
    ) -> Result<(), PluginManagerOperationError> {
        let snapshot = self
            .player
            .snapshot()
            .await
            .map_err(|error| manager_error(operation, error.to_string()))?;
        let should_resume = snapshot.state == PlaybackState::Playing;
        self.resume_after_change
            .store(should_resume, Ordering::Release);
        if should_resume {
            self.player
                .pause()
                .await
                .map_err(|error| manager_error(operation, error.to_string()))?;
        }
        Ok(())
    }

    async fn finish_change(
        &self,
        operation: &'static str,
    ) -> Result<(), PluginManagerOperationError> {
        if self
            .player
            .snapshot()
            .await
            .ok()
            .and_then(|snapshot| snapshot.current_item_id)
            .is_some()
        {
            self.player
                .rebuild_output()
                .await
                .map_err(|error| manager_error(operation, error.to_string()))?;
        }
        if self.resume_after_change.swap(false, Ordering::AcqRel) {
            self.player
                .play()
                .await
                .map_err(|error| manager_error(operation, error.to_string()))?;
        }
        Ok(())
    }

    async fn abort_change(&self) {
        if self.resume_after_change.swap(false, Ordering::AcqRel) {
            let _ = self.player.play().await;
        }
    }

    async fn install(
        &self,
        artifact_path: PathBuf,
    ) -> Result<InstalledV2PluginSummary, PluginManagerOperationError> {
        const OPERATION: &str = "install";
        self.begin_change(OPERATION).await?;
        // Package identity can live inside a ZIP, so stop every lazy process
        // before staging. Install/update is infrequent and this guarantees the
        // target package has no open Node module handles before replacement.
        if let Err(error) = self.runtime.shutdown().await {
            self.abort_change().await;
            return Err(manager_error(OPERATION, error.to_string()));
        }
        let plugins_dir = self.plugins_dir.clone();
        let installed = tokio::task::spawn_blocking(move || {
            install_typescript_artifact(&plugins_dir, &artifact_path)
        })
        .await
        .map_err(|error| manager_error(OPERATION, error.to_string()))
        .and_then(|result| result.map_err(|error| manager_error(OPERATION, error.to_string())));
        let installed = match installed {
            Ok(installed) => installed,
            Err(error) => {
                self.abort_change().await;
                return Err(error);
            },
        };
        if let Err(error) = self
            .runtime
            .register(installed.manifest.clone(), &installed.root_dir)
            .await
        {
            self.abort_change().await;
            return Err(manager_error(OPERATION, error.to_string()));
        }
        self.finish_change(OPERATION).await?;
        Ok(summary(&installed))
    }

    async fn set_enabled(
        &self,
        plugin_id: String,
        enabled: bool,
    ) -> Result<(), PluginManagerOperationError> {
        const OPERATION: &str = "set-enabled";
        self.begin_change(OPERATION).await?;
        let result = if enabled {
            let plugins_dir = self.plugins_dir.clone();
            let requested = plugin_id.clone();
            let plugin = tokio::task::spawn_blocking(move || {
                discover_typescript_plugins(&plugins_dir).map(|plugins| {
                    plugins
                        .into_iter()
                        .find(|plugin| plugin.manifest.id == requested)
                })
            })
            .await
            .map_err(|error| manager_error(OPERATION, error.to_string()))
            .and_then(|result| result.map_err(|error| manager_error(OPERATION, error.to_string())))?
            .ok_or_else(|| manager_error(OPERATION, format!("plugin '{plugin_id}' not found")))?;
            self.runtime
                .register(plugin.manifest, plugin.root_dir)
                .await
        } else {
            self.runtime.unregister(&plugin_id).await
        };
        if let Err(error) = result {
            self.abort_change().await;
            return Err(manager_error(OPERATION, error.to_string()));
        }
        self.finish_change(OPERATION).await
    }

    async fn uninstall(&self, plugin_id: String) -> Result<(), PluginManagerOperationError> {
        const OPERATION: &str = "uninstall";
        self.begin_change(OPERATION).await?;
        if let Err(error) = self.runtime.unregister(&plugin_id).await {
            self.abort_change().await;
            return Err(manager_error(OPERATION, error.to_string()));
        }
        let plugins_dir = self.plugins_dir.clone();
        let result = tokio::task::spawn_blocking(move || {
            uninstall_typescript_plugin(&plugins_dir, &plugin_id)
        })
        .await
        .map_err(|error| manager_error(OPERATION, error.to_string()))
        .and_then(|result| result.map_err(|error| manager_error(OPERATION, error.to_string())));
        if let Err(error) = result {
            self.abort_change().await;
            return Err(error);
        }
        self.finish_change(OPERATION).await
    }
}

#[derive(Clone)]
pub struct PluginManagerHandle {
    actor: ActorHandle<PluginManagerActor>,
    timeout: Duration,
}

impl PluginManagerHandle {
    pub fn spawn(
        player: PlaybackController,
        runtime: Arc<TypeScriptRuntime>,
        plugins_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            actor: spawn_actor(
                PluginManagerActor {
                    player,
                    resume_after_change: AtomicBool::new(false),
                    runtime,
                    plugins_dir: plugins_dir.into(),
                },
                MailboxConfig::bounded(16),
            ),
            timeout: Duration::from_secs(30),
        }
    }

    pub async fn discover(
        &self,
    ) -> Result<Vec<InstalledV2PluginSummary>, PluginManagerOperationError> {
        self.actor
            .ask(DiscoverPlugins, self.timeout)
            .await
            .map_err(|error| manager_error("discover", error.to_string()))?
    }

    pub async fn reconcile(
        &self,
        disabled_plugin_ids: HashSet<String>,
    ) -> Result<Vec<InstalledV2PluginSummary>, PluginManagerOperationError> {
        self.actor
            .ask(
                ReconcilePlugins {
                    disabled_plugin_ids,
                },
                self.timeout,
            )
            .await
            .map_err(|error| manager_error("reconcile", error.to_string()))?
    }

    pub async fn install(
        &self,
        artifact_path: impl Into<PathBuf>,
    ) -> Result<InstalledV2PluginSummary, PluginManagerOperationError> {
        self.actor
            .ask(
                InstallPlugin {
                    artifact_path: artifact_path.into(),
                },
                self.timeout,
            )
            .await
            .map_err(|error| manager_error("install", error.to_string()))?
    }

    pub async fn set_enabled(
        &self,
        plugin_id: impl Into<String>,
        enabled: bool,
    ) -> Result<(), PluginManagerOperationError> {
        self.actor
            .ask(
                SetPluginEnabled {
                    plugin_id: plugin_id.into(),
                    enabled,
                },
                self.timeout,
            )
            .await
            .map_err(|error| manager_error("set-enabled", error.to_string()))?
    }

    pub async fn uninstall(
        &self,
        plugin_id: impl Into<String>,
    ) -> Result<(), PluginManagerOperationError> {
        self.actor
            .ask(
                UninstallPlugin {
                    plugin_id: plugin_id.into(),
                },
                self.timeout,
            )
            .await
            .map_err(|error| manager_error("uninstall", error.to_string()))?
    }

    pub fn stop(&self) -> Result<(), PluginManagerOperationError> {
        self.actor
            .stop(StopReason::Requested)
            .map_err(|error| manager_error("stop", error.to_string()))
    }
}

fn summary(plugin: &InstalledTypeScriptPlugin) -> InstalledV2PluginSummary {
    InstalledV2PluginSummary {
        id: plugin.manifest.id.clone(),
        version: plugin.manifest.version.clone(),
    }
}

fn manager_error(operation: &'static str, message: String) -> PluginManagerOperationError {
    PluginManagerOperationError { operation, message }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use serde_json::json;
    use stellatune_plugins::typescript::TypeScriptRuntime;

    use super::PluginManagerHandle;
    use crate::runtime::shared_playback_controller;

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("backend crate must be under repository/crates")
            .to_path_buf()
    }

    #[tokio::test]
    async fn package_transactions_stop_process_and_leave_no_active_version_lease() {
        let root = repository_root();
        let fixture = root.join("tools/typescript-plugin-runtime/fixtures");
        let plugins = tempfile::tempdir().unwrap();
        let player = shared_playback_controller();
        let runtime = Arc::new(TypeScriptRuntime::new(
            root.join("tools/typescript-plugin-runtime/runner.mjs"),
        ));
        let manager = PluginManagerHandle::spawn(player, Arc::clone(&runtime), plugins.path());

        let installed = manager.install(&fixture).await.unwrap();
        assert_eq!(installed.id, "dev.stellatune.fixture.http-source");
        assert!(
            runtime
                .process_snapshot(&installed.id)
                .await
                .unwrap()
                .is_none()
        );

        runtime
            .invoke(
                &installed.id,
                "fixture-search",
                None,
                "echo",
                json!({"value": 1}),
                None,
            )
            .await
            .unwrap();
        assert!(
            runtime
                .process_snapshot(&installed.id)
                .await
                .unwrap()
                .unwrap()
                .running
        );

        manager.set_enabled(&installed.id, false).await.unwrap();
        assert!(
            runtime
                .process_snapshot(&installed.id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            runtime
                .invoke(
                    &installed.id,
                    "fixture-search",
                    None,
                    "echo",
                    json!(null),
                    None
                )
                .await
                .is_err()
        );

        manager.set_enabled(&installed.id, true).await.unwrap();
        assert!(
            runtime
                .process_snapshot(&installed.id)
                .await
                .unwrap()
                .is_none()
        );
        manager.uninstall(&installed.id).await.unwrap();
        assert!(manager.discover().await.unwrap().is_empty());
        assert!(
            runtime
                .process_snapshot(&installed.id)
                .await
                .unwrap()
                .is_none()
        );

        manager.stop().unwrap();
    }
}
