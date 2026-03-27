use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use std::thread;

use anyhow::Result;

use crate::executor::WasmtimePluginController;
use crate::host::http::HttpClientHost;
use crate::host::stream::DefaultHostStreamService;
use crate::manifest::AbilityKind;
use crate::runtime::model::RuntimeCapabilityDescriptor;

use super::service::SharedPluginRuntime;
use super::{RuntimeCapabilityKind, WasmPluginError};

pub fn shared_runtime_service() -> SharedPluginRuntime {
    static SHARED: OnceLock<SharedPluginRuntime> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            SharedPluginRuntime::new().expect("failed to initialize shared wasm plugin runtime")
        })
        .clone()
}

pub(super) fn build_shared_runtime() -> Result<SharedPluginRuntime> {
    let controller = WasmtimePluginController::shared(
        Arc::new(BackendHttpClient),
        Arc::new(DefaultHostStreamService),
    )
    .map_err(|error| anyhow::anyhow!("failed to create wasmtime plugin controller: {error:#}"))?;
    Ok(SharedPluginRuntime::from_controller(controller))
}

pub(super) fn normalize_ext(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

pub(super) fn decoder_score_for_ext(capability: &RuntimeCapabilityDescriptor, ext: &str) -> u16 {
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

pub(super) fn map_ability_kind(kind: AbilityKind) -> RuntimeCapabilityKind {
    match kind {
        AbilityKind::Decoder => RuntimeCapabilityKind::Decoder,
        AbilityKind::Encoder => RuntimeCapabilityKind::Encoder,
        AbilityKind::Dsp => RuntimeCapabilityKind::Dsp,
        AbilityKind::Source => RuntimeCapabilityKind::SourceCatalog,
        AbilityKind::Lyrics => RuntimeCapabilityKind::LyricsProvider,
        AbilityKind::OutputSink => RuntimeCapabilityKind::OutputSink,
    }
}

pub(super) fn normalize_disabled_ids(
    disabled_ids: HashSet<String>,
) -> std::collections::BTreeMap<String, crate::runtime::model::DesiredPluginState> {
    let mut desired =
        std::collections::BTreeMap::<String, crate::runtime::model::DesiredPluginState>::new();
    for plugin_id in disabled_ids {
        let plugin_id = plugin_id.trim();
        if plugin_id.is_empty() {
            continue;
        }
        desired.insert(
            plugin_id.to_string(),
            crate::runtime::model::DesiredPluginState::Disabled,
        );
    }
    desired
}

#[derive(Default)]
struct BackendHttpClient;

impl HttpClientHost for BackendHttpClient {
    fn fetch_json(&self, url: &str) -> std::result::Result<String, WasmPluginError> {
        let url = url.trim();
        if url.is_empty() {
            return Err(WasmPluginError::invalid_input("url is empty"));
        }
        let url = url.to_string();
        let worker = thread::Builder::new()
            .name("stellatune-http-client".to_string())
            .spawn(move || {
                reqwest::blocking::get(url)
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
                    })
            })
            .map_err(|error| {
                WasmPluginError::operation(
                    "http_client.fetch_json",
                    format!("spawn blocking worker failed: {error}"),
                )
            })?;
        worker.join().map_err(|_| {
            WasmPluginError::operation(
                "http_client.fetch_json",
                "blocking worker panicked".to_string(),
            )
        })?
    }
}
