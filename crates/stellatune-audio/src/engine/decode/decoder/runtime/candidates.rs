use std::collections::HashSet;
use std::path::Path;

use stellatune_plugins::runtime::introspection::CapabilityKind as RuntimeCapabilityKind;

use super::DecoderCandidate;

fn normalize_ext_hint(raw: &str) -> String {
    raw.trim().trim_start_matches('.').to_ascii_lowercase()
}

pub(super) fn ext_hint_from_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(normalize_ext_hint)
        .unwrap_or_default()
}

fn runtime_scored_decoder_candidates(ext_hint: &str) -> Vec<DecoderCandidate> {
    let ext = normalize_ext_hint(ext_hint);
    if ext.is_empty() {
        return Vec::new();
    }
    let service = stellatune_plugins::runtime::handle::shared_runtime_service();
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for candidate in stellatune_runtime::block_on(service.list_decoder_candidates_for_ext(&ext)) {
        if !seen.insert((candidate.plugin_id.clone(), candidate.type_id.clone())) {
            continue;
        }
        let Some(cap) = stellatune_runtime::block_on(service.find_capability(
            &candidate.plugin_id,
            RuntimeCapabilityKind::Decoder,
            &candidate.type_id,
        )) else {
            continue;
        };
        out.push(DecoderCandidate {
            plugin_id: candidate.plugin_id,
            type_id: candidate.type_id,
            default_config_json: cap.default_config_json,
        });
    }
    out
}

fn runtime_all_decoder_candidates() -> Vec<DecoderCandidate> {
    let service = stellatune_plugins::runtime::handle::shared_runtime_service();
    let mut plugin_ids = stellatune_runtime::block_on(service.active_plugin_ids());
    plugin_ids.sort();
    let mut out = Vec::new();
    for plugin_id in plugin_ids {
        let mut caps = stellatune_runtime::block_on(service.list_capabilities(&plugin_id));
        caps.sort_by(|a, b| a.type_id.cmp(&b.type_id));
        for cap in caps {
            if cap.kind != RuntimeCapabilityKind::Decoder {
                continue;
            }
            out.push(DecoderCandidate {
                plugin_id: plugin_id.clone(),
                type_id: cap.type_id,
                default_config_json: cap.default_config_json,
            });
        }
    }
    out
}

pub(super) fn select_decoder_candidates(
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> Result<Vec<DecoderCandidate>, String> {
    match (decoder_plugin_id, decoder_type_id) {
        (Some(plugin_id), Some(type_id)) => {
            let service = stellatune_plugins::runtime::handle::shared_runtime_service();
            let cap = stellatune_runtime::block_on(service.find_capability(
                plugin_id,
                RuntimeCapabilityKind::Decoder,
                type_id,
            ))
            .ok_or_else(|| {
                format!(
                    "decoder not found for source track: plugin_id={} type_id={}",
                    plugin_id, type_id
                )
            })?;
            Ok(vec![DecoderCandidate {
                plugin_id: plugin_id.to_string(),
                type_id: type_id.to_string(),
                default_config_json: cap.default_config_json,
            }])
        }
        (Some(plugin_id), None) | (None, Some(plugin_id)) => Err(format!(
            "invalid decoder selector: both plugin_id and type_id are required, got `{plugin_id}` only"
        )),
        (None, None) => {
            let mut out = runtime_scored_decoder_candidates(ext_hint);
            if out.is_empty() {
                out = runtime_all_decoder_candidates();
            }
            if out.is_empty() {
                Err("no v2 decoder candidates available".to_string())
            } else {
                Ok(out)
            }
        }
    }
}

pub(super) fn has_decoder_candidates(
    ext_hint: &str,
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> bool {
    !select_decoder_candidates(ext_hint, decoder_plugin_id, decoder_type_id)
        .unwrap_or_default()
        .is_empty()
}

pub(super) fn runtime_has_source_catalog(plugin_id: &str, type_id: &str) -> bool {
    stellatune_runtime::block_on(
        stellatune_plugins::runtime::handle::shared_runtime_service().find_capability(
            plugin_id,
            RuntimeCapabilityKind::SourceCatalog,
            type_id,
        ),
    )
    .is_some()
}
