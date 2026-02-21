use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};

use crate::executor::WasmtimePluginController;
use crate::host::http::HttpClientHost;
use crate::host::stream::DefaultHostStreamService;
use crate::manifest::AbilityKind;
use crate::runtime::model::{
    DesiredPluginState, RuntimePluginLifecycleState, RuntimePluginTransitionOutcome,
    RuntimePluginTransitionTrigger,
};
use crate::runtime::service::WasmPluginRuntime;
use anyhow::{Result, anyhow};

use super::{
    RUNTIME_DECODER_PLUGIN_SEQ, RUNTIME_DECODER_PLUGINS, RUNTIME_DSP_PLUGIN_SEQ,
    RUNTIME_DSP_PLUGINS, RUNTIME_OUTPUT_SINK_PLUGIN_SEQ, RUNTIME_OUTPUT_SINK_PLUGINS,
    RuntimeCapabilityDescriptor, RuntimeCapabilityKind, RuntimeDecoderCandidate,
    RuntimeDecoderPlugin, RuntimeDecoderPluginCell, RuntimeDspPlugin, RuntimeDspPluginCell,
    RuntimeLyricsPlugin, RuntimeOutputSinkPlugin, RuntimeOutputSinkPluginCell, RuntimeSourcePlugin,
    WasmPluginError,
};

#[derive(Debug, Clone)]
pub struct RuntimeActivePluginDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Default)]
pub struct RuntimeLoadReport {
    pub loaded: Vec<String>,
    pub deactivated: Vec<String>,
    pub errors: Vec<anyhow::Error>,
}

#[derive(Debug, Default)]
pub struct RuntimeSyncPlanSummary {
    pub discovered: usize,
    pub disabled: usize,
    pub actions_total: usize,
    pub load_new: usize,
    pub reload_changed: usize,
    pub deactivate: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeSyncActionOutcome {
    pub action: String,
    pub plugin_id: String,
    pub outcome: String,
}

#[derive(Debug, Default)]
pub struct RuntimeSyncReport {
    pub load_report: RuntimeLoadReport,
    pub plan: RuntimeSyncPlanSummary,
    pub actions: Vec<RuntimeSyncActionOutcome>,
    pub plan_ms: u64,
    pub execute_ms: u64,
    pub total_ms: u64,
}

#[derive(Clone)]
pub struct SharedPluginRuntime {
    runtime: WasmPluginRuntime<WasmtimePluginController>,
}

impl SharedPluginRuntime {
    pub fn new() -> Result<Self> {
        let controller = WasmtimePluginController::shared(
            Arc::new(BackendHttpClient),
            Arc::new(DefaultHostStreamService),
        )
        .map_err(|error| anyhow!("failed to create wasmtime plugin controller: {error:#}"))?;
        Ok(Self {
            runtime: WasmPluginRuntime::new(controller),
        })
    }

    pub async fn set_disabled_plugin_ids(&self, disabled_ids: HashSet<String>) {
        let mut normalized_ids = disabled_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        normalized_ids.sort();
        tracing::debug!(
            target: "stellatune_plugins::runtime",
            disabled_plugins = ?normalized_ids,
            "host runtime apply disabled plugin ids"
        );
        let mut desired = BTreeMap::<String, DesiredPluginState>::new();
        for plugin_id in disabled_ids {
            let plugin_id = plugin_id.trim();
            if plugin_id.is_empty() {
                continue;
            }
            desired.insert(plugin_id.to_string(), DesiredPluginState::Disabled);
        }
        if let Err(error) = self.runtime.replace_desired_states(desired) {
            tracing::warn!(
                error = %error,
                "failed to apply desired plugin states for wasm runtime"
            );
        }
    }

    pub fn active_plugins_snapshot(&self) -> Vec<RuntimeActivePluginDescriptor> {
        let mut out = self
            .runtime
            .active_plugins()
            .into_iter()
            .map(|plugin| RuntimeActivePluginDescriptor {
                id: plugin.id,
                name: plugin.name,
                version: plugin.version,
            })
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub async fn active_plugins(&self) -> Vec<RuntimeActivePluginDescriptor> {
        self.active_plugins_snapshot()
    }

    pub fn list_decoder_candidates_for_ext(&self, ext: &str) -> Vec<RuntimeDecoderCandidate> {
        let ext = normalize_ext(ext);
        let mut out = Vec::<RuntimeDecoderCandidate>::new();
        for plugin_id in self.decoder_capability_plugin_ids() {
            for cap in self.runtime.capabilities_of(&plugin_id) {
                if cap.kind != AbilityKind::Decoder {
                    continue;
                }
                let score = decoder_score_for_ext(&cap, ext.as_str());
                if score == 0 {
                    continue;
                }
                out.push(RuntimeDecoderCandidate {
                    plugin_id: plugin_id.clone(),
                    type_id: cap.type_id,
                    score,
                });
            }
        }
        out.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.plugin_id.cmp(&b.plugin_id))
                .then_with(|| a.type_id.cmp(&b.type_id))
        });
        out
    }

    pub fn decoder_supported_extensions(&self) -> Vec<String> {
        let mut out = BTreeSet::<String>::new();
        for plugin_id in self.decoder_capability_plugin_ids() {
            for cap in self.runtime.capabilities_of(&plugin_id) {
                if cap.kind != AbilityKind::Decoder {
                    continue;
                }
                for rule in cap.decoder_ext_scores {
                    let ext = normalize_ext(rule.ext.as_str());
                    if ext.is_empty() {
                        continue;
                    }
                    out.insert(ext);
                }
            }
        }
        out.into_iter().collect()
    }

    pub fn decoder_has_wildcard_candidate(&self) -> bool {
        for plugin_id in self.decoder_capability_plugin_ids() {
            for cap in self.runtime.capabilities_of(&plugin_id) {
                if cap.kind != AbilityKind::Decoder {
                    continue;
                }
                if cap.decoder_wildcard_score > 0 {
                    return true;
                }
            }
        }
        false
    }

    pub fn decoder_capability_plugin_ids(&self) -> Vec<String> {
        let mut out = self.runtime.active_ids();
        out.retain(|plugin_id| {
            self.runtime
                .capabilities_of(plugin_id)
                .into_iter()
                .any(|cap| cap.kind == AbilityKind::Decoder)
        });
        out.sort();
        out
    }

    pub fn list_capabilities(&self, plugin_id: &str) -> Vec<RuntimeCapabilityDescriptor> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Vec::new();
        }
        let mut out = self
            .runtime
            .capabilities_of(plugin_id)
            .into_iter()
            .map(|cap| RuntimeCapabilityDescriptor {
                kind: map_ability_kind(cap.kind),
                type_id: cap.type_id,
                display_name: cap.display_name,
                config_schema_json: cap.config_schema_json,
                default_config_json: cap.default_config_json,
            })
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        out
    }

    pub fn list_capabilities_snapshot(&self, plugin_id: &str) -> Vec<RuntimeCapabilityDescriptor> {
        self.list_capabilities(plugin_id)
    }

    fn list_capabilities_of_kind(
        &self,
        plugin_id: &str,
        kind: RuntimeCapabilityKind,
    ) -> Vec<RuntimeCapabilityDescriptor> {
        self.list_capabilities(plugin_id)
            .into_iter()
            .filter(|cap| cap.kind == kind)
            .collect()
    }

    pub fn list_decoder_capabilities(&self, plugin_id: &str) -> Vec<RuntimeCapabilityDescriptor> {
        self.list_capabilities_of_kind(plugin_id, RuntimeCapabilityKind::Decoder)
    }

    pub fn list_dsp_capabilities(&self, plugin_id: &str) -> Vec<RuntimeCapabilityDescriptor> {
        self.list_capabilities_of_kind(plugin_id, RuntimeCapabilityKind::Dsp)
    }

    pub fn list_source_capabilities(&self, plugin_id: &str) -> Vec<RuntimeCapabilityDescriptor> {
        self.list_capabilities_of_kind(plugin_id, RuntimeCapabilityKind::SourceCatalog)
    }

    pub fn list_lyrics_capabilities(&self, plugin_id: &str) -> Vec<RuntimeCapabilityDescriptor> {
        self.list_capabilities_of_kind(plugin_id, RuntimeCapabilityKind::LyricsProvider)
    }

    pub fn list_output_sink_capabilities(
        &self,
        plugin_id: &str,
    ) -> Vec<RuntimeCapabilityDescriptor> {
        self.list_capabilities_of_kind(plugin_id, RuntimeCapabilityKind::OutputSink)
    }

    pub fn find_capability(
        &self,
        plugin_id: &str,
        kind: RuntimeCapabilityKind,
        type_id: &str,
    ) -> Option<RuntimeCapabilityDescriptor> {
        let plugin_id = plugin_id.trim();
        let type_id = type_id.trim();
        if plugin_id.is_empty() || type_id.is_empty() {
            return None;
        }
        self.list_capabilities(plugin_id)
            .into_iter()
            .find(|cap| cap.kind == kind && cap.type_id == type_id)
    }

    pub fn create_decoder_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<RuntimeDecoderPlugin> {
        let decoder = self
            .runtime
            .controller()
            .create_decoder_plugin(plugin_id, type_id)
            .map_err(|error| anyhow!(error.to_string()))?;
        let id = RUNTIME_DECODER_PLUGIN_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
        RUNTIME_DECODER_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            map.insert(id, RuntimeDecoderPluginCell { inner: decoder });
        });
        Ok(RuntimeDecoderPlugin { id })
    }

    pub fn create_source_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<RuntimeSourcePlugin> {
        let source = self
            .runtime
            .controller()
            .create_source_plugin(plugin_id, type_id)
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(RuntimeSourcePlugin { inner: source })
    }

    pub fn create_dsp_plugin(&self, plugin_id: &str, type_id: &str) -> Result<RuntimeDspPlugin> {
        let dsp = self
            .runtime
            .controller()
            .create_dsp_plugin(plugin_id, type_id)
            .map_err(|error| anyhow!(error.to_string()))?;
        let id = RUNTIME_DSP_PLUGIN_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
        RUNTIME_DSP_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            map.insert(
                id,
                RuntimeDspPluginCell {
                    inner: dsp,
                    processor: None,
                    spec: None,
                },
            );
        });
        Ok(RuntimeDspPlugin { id })
    }

    pub fn create_output_sink_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<RuntimeOutputSinkPlugin> {
        let output = self
            .runtime
            .controller()
            .create_output_sink_plugin(plugin_id, type_id)
            .map_err(|error| anyhow!(error.to_string()))?;
        let id = RUNTIME_OUTPUT_SINK_PLUGIN_SEQ.fetch_add(1, Ordering::Relaxed) + 1;
        RUNTIME_OUTPUT_SINK_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            map.insert(id, RuntimeOutputSinkPluginCell { inner: output });
        });
        Ok(RuntimeOutputSinkPlugin { id })
    }

    pub fn create_lyrics_plugin(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<RuntimeLyricsPlugin> {
        let lyrics = self
            .runtime
            .controller()
            .create_lyrics_plugin(plugin_id, type_id)
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(RuntimeLyricsPlugin { inner: lyrics })
    }

    pub fn active_plugin_ids(&self) -> Vec<String> {
        self.runtime.active_ids()
    }

    pub fn sync_dir_with_disabled_ids(
        &self,
        dir: impl AsRef<Path>,
        disabled_ids: HashSet<String>,
    ) -> Result<()> {
        let mut desired = BTreeMap::<String, DesiredPluginState>::new();
        for plugin_id in disabled_ids {
            let plugin_id = plugin_id.trim();
            if plugin_id.is_empty() {
                continue;
            }
            desired.insert(plugin_id.to_string(), DesiredPluginState::Disabled);
        }
        self.runtime
            .replace_desired_states(desired)
            .map_err(|error| anyhow!(error.to_string()))?;
        self.runtime
            .sync_plugins(dir)
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(())
    }

    pub fn unload_plugin(&self, plugin_id: &str) -> Result<()> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return Ok(());
        }
        self.runtime
            .uninstall_plugin(plugin_id)
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<()> {
        self.runtime
            .shutdown()
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(())
    }

    pub async fn unload_plugin_report(&self, plugin_id: &str) -> RuntimeLoadReport {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return RuntimeLoadReport::default();
        }
        match self.runtime.uninstall_plugin(plugin_id) {
            Ok(unloaded) => RuntimeLoadReport {
                loaded: Vec::new(),
                deactivated: if unloaded {
                    vec![plugin_id.to_string()]
                } else {
                    Vec::new()
                },
                errors: Vec::new(),
            },
            Err(error) => RuntimeLoadReport {
                loaded: Vec::new(),
                deactivated: Vec::new(),
                errors: vec![anyhow!(error.to_string())],
            },
        }
    }

    pub async fn shutdown_and_cleanup(&self) -> RuntimeLoadReport {
        match self.runtime.shutdown() {
            Ok(deactivated) => RuntimeLoadReport {
                loaded: Vec::new(),
                deactivated,
                errors: Vec::new(),
            },
            Err(error) => RuntimeLoadReport {
                loaded: Vec::new(),
                deactivated: Vec::new(),
                errors: vec![anyhow!(error.to_string())],
            },
        }
    }

    pub async fn reload_dir_detailed_from_state(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<RuntimeSyncReport> {
        let dir = dir.as_ref();
        tracing::debug!(
            target: "stellatune_plugins::runtime",
            plugins_dir = %dir.display(),
            "host runtime reload from state begin"
        );
        let report = self
            .runtime
            .sync_plugins(dir)
            .map_err(|error| anyhow!(error.to_string()))?;

        let mut load_new = 0usize;
        let mut reload_changed = 0usize;
        let mut deactivate = 0usize;
        let mut loaded = Vec::<String>::new();
        let mut deactivated = Vec::<String>::new();
        let mut actions = Vec::<RuntimeSyncActionOutcome>::new();

        for transition in &report.transitions {
            match (transition.trigger, transition.outcome) {
                (
                    RuntimePluginTransitionTrigger::LoadNew,
                    RuntimePluginTransitionOutcome::Applied,
                ) => {
                    load_new = load_new.saturating_add(1);
                    loaded.push(transition.plugin_id.clone());
                    actions.push(RuntimeSyncActionOutcome {
                        action: "load_new".to_string(),
                        plugin_id: transition.plugin_id.clone(),
                        outcome: "ok".to_string(),
                    });
                },
                (
                    RuntimePluginTransitionTrigger::ReloadChanged,
                    RuntimePluginTransitionOutcome::Applied,
                ) => {
                    reload_changed = reload_changed.saturating_add(1);
                    loaded.push(transition.plugin_id.clone());
                    actions.push(RuntimeSyncActionOutcome {
                        action: "reload_changed".to_string(),
                        plugin_id: transition.plugin_id.clone(),
                        outcome: "ok".to_string(),
                    });
                },
                (
                    RuntimePluginTransitionTrigger::DisableRequested,
                    RuntimePluginTransitionOutcome::Applied,
                )
                | (
                    RuntimePluginTransitionTrigger::RemovedFromDisk,
                    RuntimePluginTransitionOutcome::Applied,
                ) => {
                    deactivate = deactivate.saturating_add(1);
                    deactivated.push(transition.plugin_id.clone());
                    actions.push(RuntimeSyncActionOutcome {
                        action: "deactivate_missing_or_disabled".to_string(),
                        plugin_id: transition.plugin_id.clone(),
                        outcome: "ok".to_string(),
                    });
                },
                (_, RuntimePluginTransitionOutcome::Skipped) => {
                    actions.push(RuntimeSyncActionOutcome {
                        action: "skip".to_string(),
                        plugin_id: transition.plugin_id.clone(),
                        outcome: transition.detail.clone(),
                    });
                },
                (_, RuntimePluginTransitionOutcome::Failed) => {
                    actions.push(RuntimeSyncActionOutcome {
                        action: "fail".to_string(),
                        plugin_id: transition.plugin_id.clone(),
                        outcome: transition.detail.clone(),
                    });
                },
            }
        }

        let disabled = report
            .plugin_statuses
            .iter()
            .filter(|status| {
                status.desired_state == DesiredPluginState::Disabled
                    && status.lifecycle_state != RuntimePluginLifecycleState::Missing
            })
            .count();

        let load_errors = report
            .errors
            .into_iter()
            .map(|error| anyhow!(error))
            .collect::<Vec<_>>();

        tracing::info!(
            target: "stellatune_plugins::runtime",
            plugins_dir = %dir.display(),
            discovered = report.discovered_plugins,
            loaded = loaded.len(),
            deactivated = deactivated.len(),
            actions_total = actions.len(),
            load_errors = load_errors.len(),
            "host runtime reload from state completed"
        );
        for action in actions
            .iter()
            .filter(|action| action.action == "fail" || action.action == "skip")
        {
            tracing::warn!(
                target: "stellatune_plugins::runtime",
                action = %action.action,
                plugin_id = %action.plugin_id,
                outcome = %action.outcome,
                "host runtime action requires attention"
            );
        }

        Ok(RuntimeSyncReport {
            load_report: RuntimeLoadReport {
                loaded,
                deactivated,
                errors: load_errors,
            },
            plan: RuntimeSyncPlanSummary {
                discovered: report.discovered_plugins,
                disabled,
                actions_total: report.transitions.len(),
                load_new,
                reload_changed,
                deactivate,
            },
            actions,
            plan_ms: 0,
            execute_ms: 0,
            total_ms: 0,
        })
    }

    pub fn active_plugin_metadata_json(&self, plugin_id: &str) -> Option<String> {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            return None;
        }
        let info = self
            .runtime
            .active_plugins()
            .into_iter()
            .find(|item| item.id == plugin_id)?;
        Some(
            serde_json::json!({
                "id": info.id,
                "name": info.name,
                "version": info.version,
            })
            .to_string(),
        )
    }
}

pub fn shared_runtime_service() -> SharedPluginRuntime {
    static SHARED: OnceLock<SharedPluginRuntime> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            SharedPluginRuntime::new().expect("failed to initialize shared wasm plugin runtime")
        })
        .clone()
}

fn normalize_ext(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn decoder_score_for_ext(
    capability: &crate::runtime::model::RuntimeCapabilityDescriptor,
    ext: &str,
) -> u16 {
    if ext.is_empty() {
        return capability.decoder_wildcard_score;
    }
    capability
        .decoder_ext_scores
        .iter()
        .find(|rule| rule.ext == ext)
        .map(|rule| rule.score)
        .unwrap_or(capability.decoder_wildcard_score)
}

fn map_ability_kind(kind: AbilityKind) -> RuntimeCapabilityKind {
    match kind {
        AbilityKind::Decoder => RuntimeCapabilityKind::Decoder,
        AbilityKind::Dsp => RuntimeCapabilityKind::Dsp,
        AbilityKind::Source => RuntimeCapabilityKind::SourceCatalog,
        AbilityKind::Lyrics => RuntimeCapabilityKind::LyricsProvider,
        AbilityKind::OutputSink => RuntimeCapabilityKind::OutputSink,
    }
}

#[derive(Default)]
struct BackendHttpClient;

impl HttpClientHost for BackendHttpClient {
    fn fetch_json(&self, url: &str) -> std::result::Result<String, WasmPluginError> {
        let url = url.trim();
        if url.is_empty() {
            return Err(WasmPluginError::invalid_input("url is empty"));
        }
        let body = reqwest::blocking::get(url)
            .map_err(|error| {
                WasmPluginError::operation("http_client.fetch_json", error.to_string())
            })?
            .error_for_status()
            .map_err(|error| {
                WasmPluginError::operation("http_client.fetch_json", error.to_string())
            })?
            .text()
            .map_err(|error| {
                WasmPluginError::operation("http_client.fetch_json", error.to_string())
            })?;
        Ok(body)
    }
}
