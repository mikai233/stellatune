use std::collections::BTreeSet;

use anyhow::anyhow;
use axum::http::StatusCode;
use serde_json::{Map, Value, json};

use crate::plugin_ui_gateway::model::{ConfigApplyOutcome, ConfigApplyReport};
use crate::runtime::{shared_plugin_runtime, shared_runtime_engine};
use stellatune_plugins::host_runtime::RuntimeCapabilityKind;

const KIND_DECODER: &str = "decoder";
const KIND_SOURCE: &str = "source";
const KIND_LYRICS: &str = "lyrics";
const KIND_OUTPUT_SINK: &str = "output_sink";
const KIND_DSP: &str = "dsp";
const ACTION_PLAYBACK_PLAY_TRACK_REF: &str = "playback.play_track_ref";
const ACTION_PLAYBACK_ENQUEUE_TRACK_REF: &str = "playback.enqueue_track_ref";
const ACTION_PLAYBACK_PAUSE: &str = "playback.pause";
const ACTION_PLAYBACK_NEXT: &str = "playback.next";
const ACTION_PLAYBACK_STOP: &str = "playback.stop";

pub(super) fn validate_config_payload(config: &Value) -> Result<(), String> {
    let Some(root) = config.as_object() else {
        return Err("config payload must be a JSON object".to_string());
    };
    let allowed_keys = BTreeSet::from([
        KIND_DECODER,
        KIND_SOURCE,
        KIND_LYRICS,
        KIND_OUTPUT_SINK,
        KIND_DSP,
    ]);
    for (kind_key, type_map) in root {
        if !allowed_keys.contains(kind_key.as_str()) {
            return Err(format!(
                "unsupported config kind `{kind_key}` (allowed: decoder/source/lyrics/output_sink/dsp)"
            ));
        }
        let Some(type_map) = type_map.as_object() else {
            return Err(format!("config kind `{kind_key}` must be an object"));
        };
        for (type_id, type_config) in type_map {
            if type_id.trim().is_empty() {
                return Err(format!("config kind `{kind_key}` contains empty type id"));
            }
            if !type_config.is_object() {
                return Err(format!(
                    "config kind `{kind_key}` type `{type_id}` must be a JSON object"
                ));
            }
        }
    }
    Ok(())
}

pub(super) async fn apply_config_best_effort(
    plugin_id: &str,
    config: &Value,
) -> Result<ConfigApplyReport, String> {
    validate_config_payload(config)?;

    let plugin_id = plugin_id.trim().to_string();
    let config = config.clone();
    tokio::task::spawn_blocking(move || apply_config_best_effort_blocking(plugin_id, config))
        .await
        .map_err(|error| format!("join error while applying plugin config: {error}"))?
}

fn apply_config_best_effort_blocking(
    plugin_id: String,
    config: Value,
) -> Result<ConfigApplyReport, String> {
    let service = shared_plugin_runtime();
    let capabilities = service.list_capabilities(plugin_id.as_str());

    let mut report = ConfigApplyReport {
        plugin_id: plugin_id.clone(),
        applied: 0,
        skipped: 0,
        failed: 0,
        outcomes: Vec::new(),
    };

    let configured_keys = collect_configured_keys(&config);
    let mut seen_keys = BTreeSet::<(String, String)>::new();

    for capability in capabilities {
        let kind_key = capability_kind_key(capability.kind).to_string();
        let Some(type_config) = lookup_kind_type_config(&config, &kind_key, &capability.type_id)
        else {
            continue;
        };
        seen_keys.insert((kind_key.clone(), capability.type_id.clone()));

        let config_json = serde_json::to_string(type_config).map_err(|error| {
            format!(
                "serialize config for {kind_key}/{} failed: {error}",
                capability.type_id
            )
        })?;

        match capability.kind {
            RuntimeCapabilityKind::SourceCatalog => {
                let outcome = service
                    .create_source_plugin(plugin_id.as_str(), capability.type_id.as_str())
                    .map_err(|error| anyhow!(error.to_string()))
                    .and_then(|mut plugin| {
                        plugin
                            .apply_config_update_json(config_json.as_str())
                            .map_err(|error| anyhow!(error.to_string()))
                    });
                push_outcome(
                    &mut report,
                    kind_key,
                    capability.type_id,
                    outcome.map(|_| ()),
                    "source config applied",
                );
            },
            RuntimeCapabilityKind::OutputSink => {
                let outcome = service
                    .create_output_sink_plugin(plugin_id.as_str(), capability.type_id.as_str())
                    .map_err(|error| anyhow!(error.to_string()))
                    .and_then(|mut plugin| {
                        plugin
                            .apply_config_update_json(config_json.as_str())
                            .map_err(|error| anyhow!(error.to_string()))
                    });
                push_outcome(
                    &mut report,
                    kind_key,
                    capability.type_id,
                    outcome.map(|_| ()),
                    "output sink config applied",
                );
            },
            RuntimeCapabilityKind::Decoder
            | RuntimeCapabilityKind::LyricsProvider
            | RuntimeCapabilityKind::Dsp => {
                report.skipped += 1;
                report.outcomes.push(ConfigApplyOutcome {
                    kind: kind_key,
                    type_id: capability.type_id,
                    status: "skipped".to_string(),
                    detail: Some(
                        "runtime hot-apply is not implemented for this capability kind".to_string(),
                    ),
                });
            },
        }
    }

    for (kind_key, type_id) in configured_keys {
        if seen_keys.contains(&(kind_key.clone(), type_id.clone())) {
            continue;
        }
        report.failed += 1;
        report.outcomes.push(ConfigApplyOutcome {
            kind: kind_key,
            type_id,
            status: "failed".to_string(),
            detail: Some("capability not found in active plugin runtime".to_string()),
        });
    }

    Ok(report)
}

fn push_outcome(
    report: &mut ConfigApplyReport,
    kind: String,
    type_id: String,
    outcome: Result<(), anyhow::Error>,
    success_message: &str,
) {
    match outcome {
        Ok(()) => {
            report.applied += 1;
            report.outcomes.push(ConfigApplyOutcome {
                kind,
                type_id,
                status: "applied".to_string(),
                detail: Some(success_message.to_string()),
            });
        },
        Err(error) => {
            report.failed += 1;
            report.outcomes.push(ConfigApplyOutcome {
                kind,
                type_id,
                status: "failed".to_string(),
                detail: Some(error.to_string()),
            });
        },
    }
}

fn collect_configured_keys(config: &Value) -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::<(String, String)>::new();
    let Some(root) = config.as_object() else {
        return out;
    };
    for (kind_key, type_map) in root {
        let Some(type_map) = type_map.as_object() else {
            continue;
        };
        for type_id in type_map.keys() {
            out.insert((kind_key.to_string(), type_id.to_string()));
        }
    }
    out
}

fn lookup_kind_type_config<'a>(
    config: &'a Value,
    kind_key: &str,
    type_id: &str,
) -> Option<&'a Value> {
    config
        .as_object()
        .and_then(|root| root.get(kind_key))
        .and_then(Value::as_object)
        .and_then(|type_map| type_map.get(type_id))
}

fn capability_kind_key(kind: RuntimeCapabilityKind) -> &'static str {
    match kind {
        RuntimeCapabilityKind::Decoder => KIND_DECODER,
        RuntimeCapabilityKind::SourceCatalog => KIND_SOURCE,
        RuntimeCapabilityKind::LyricsProvider => KIND_LYRICS,
        RuntimeCapabilityKind::OutputSink => KIND_OUTPUT_SINK,
        RuntimeCapabilityKind::Dsp => KIND_DSP,
    }
}

pub(super) fn action_payload_config(payload: &Value) -> Option<&Value> {
    payload
        .as_object()
        .and_then(|obj| obj.get("config"))
        .filter(|value| value.is_object())
}

pub(super) fn action_payload_persist(payload: &Value) -> bool {
    payload
        .as_object()
        .and_then(|obj| obj.get("persist"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn build_apply_report_json(report: &ConfigApplyReport) -> Value {
    serde_json::to_value(report).unwrap_or_else(|_| {
        json!({
            "plugin_id": report.plugin_id,
            "applied": report.applied,
            "skipped": report.skipped,
            "failed": report.failed,
        })
    })
}

pub(super) fn normalize_action_payload(payload: Value) -> Value {
    if payload.is_null() {
        return Value::Object(Map::new());
    }
    payload
}

pub(super) fn build_unknown_action_error(action: &str) -> String {
    format!(
        "unsupported action `{action}`; supported: config.apply, config.get, playback.* host actions, or source-dispatchable custom actions"
    )
}

pub(super) async fn invoke_action_via_host(
    action: &str,
    payload: &Value,
) -> Result<Option<Value>, (StatusCode, String)> {
    let action = action.trim();
    if action.is_empty() {
        return Ok(None);
    }

    let engine = shared_runtime_engine();
    match action {
        ACTION_PLAYBACK_PLAY_TRACK_REF => {
            let track_token = extract_track_token_from_payload(payload, action)?;
            engine
                .switch_track_token(track_token.clone(), true)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("host action `{action}` failed: {error}"),
                    )
                })?;
            Ok(Some(json!({
                "dispatch": "host.playback",
                "action": action,
                "autoplay": true,
                "track_token": track_token,
            })))
        },
        ACTION_PLAYBACK_ENQUEUE_TRACK_REF => {
            let track_token = extract_track_token_from_payload(payload, action)?;
            engine
                .queue_next_track_token(track_token.clone())
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("host action `{action}` failed: {error}"),
                    )
                })?;
            Ok(Some(json!({
                "dispatch": "host.playback",
                "action": action,
                "queued": true,
                "track_token": track_token,
            })))
        },
        ACTION_PLAYBACK_PAUSE => {
            engine.pause().await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("host action `{action}` failed: {error}"),
                )
            })?;
            Ok(Some(json!({
                "dispatch": "host.playback",
                "action": action,
                "paused": true,
            })))
        },
        ACTION_PLAYBACK_NEXT => {
            let track_token = extract_track_token_from_payload(payload, action)?;
            engine
                .switch_track_token(track_token.clone(), true)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("host action `{action}` failed: {error}"),
                    )
                })?;
            Ok(Some(json!({
                "dispatch": "host.playback",
                "action": action,
                "mode": "explicit_track",
                "track_token": track_token,
            })))
        },
        ACTION_PLAYBACK_STOP => {
            engine.stop().await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("host action `{action}` failed: {error}"),
                )
            })?;
            Ok(Some(json!({
                "dispatch": "host.playback",
                "action": action,
                "stopped": true,
            })))
        },
        _ => Ok(None),
    }
}

pub(super) async fn invoke_action_via_source(
    plugin_id: &str,
    action: &str,
    payload: &Value,
    config_root: &Value,
) -> Result<Option<Value>, String> {
    let plugin_id = plugin_id.trim();
    let action = action.trim();
    if plugin_id.is_empty() || action.is_empty() {
        return Ok(None);
    }

    let plugin_id = plugin_id.to_string();
    let action = action.to_string();
    let payload = payload.clone();
    let config_root = config_root.clone();
    tokio::task::spawn_blocking(move || {
        invoke_action_via_source_blocking(
            plugin_id.as_str(),
            action.as_str(),
            &payload,
            &config_root,
        )
    })
    .await
    .map_err(|error| format!("join error while invoking source action: {error}"))?
}

fn invoke_action_via_source_blocking(
    plugin_id: &str,
    action: &str,
    payload: &Value,
    config_root: &Value,
) -> Result<Option<Value>, String> {
    let service = shared_plugin_runtime();
    let source_caps = service.list_source_capabilities(plugin_id);
    if source_caps.is_empty() {
        return Ok(None);
    }

    let type_id = resolve_source_type_id(action, payload, &source_caps)?;
    let mut source = service
        .create_source_plugin(plugin_id, type_id.as_str())
        .map_err(|error| {
            format!("create source plugin failed for `{plugin_id}/{type_id}`: {error}")
        })?;

    if let Some(source_config) = lookup_source_config(config_root, type_id.as_str()) {
        let config_json = serde_json::to_string(source_config)
            .map_err(|error| format!("serialize source config failed: {error}"))?;
        source
            .apply_config_update_json(config_json.as_str())
            .map_err(|error| format!("apply source config failed: {error}"))?;
    }

    let mut request = build_action_request(payload);
    request.insert("action".to_string(), Value::String(action.to_string()));
    let request_json = serde_json::to_string(&Value::Object(request))
        .map_err(|error| format!("serialize source action request failed: {error}"))?;
    let raw = source
        .list_items_json(request_json.as_str())
        .map_err(|error| format!("source action dispatch failed: {error}"))?;
    let response = serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw));

    Ok(Some(json!({
        "dispatch": "source.list_items_json",
        "source_type_id": type_id,
        "response": response
    })))
}

fn resolve_source_type_id(
    action: &str,
    payload: &Value,
    source_caps: &[stellatune_plugins::host_runtime::RuntimeCapabilityDescriptor],
) -> Result<String, String> {
    if let Some(provided) = action_payload_type_id(payload) {
        let exists = source_caps.iter().any(|cap| cap.type_id == provided);
        if !exists {
            return Err(format!(
                "source type_id `{provided}` not found in runtime capabilities"
            ));
        }
        return Ok(provided.to_string());
    }

    if source_caps.len() == 1 {
        return Ok(source_caps[0].type_id.clone());
    }

    let action_prefix = action
        .split_once('.')
        .map(|(prefix, _)| prefix)
        .unwrap_or(action);
    if let Some(matched) = source_caps
        .iter()
        .find(|cap| cap.type_id.eq_ignore_ascii_case(action_prefix))
    {
        return Ok(matched.type_id.clone());
    }

    Err("source action is ambiguous; provide payload.type_id".to_string())
}

fn lookup_source_config<'a>(config_root: &'a Value, type_id: &str) -> Option<&'a Value> {
    config_root
        .as_object()
        .and_then(|root| root.get(KIND_SOURCE))
        .and_then(Value::as_object)
        .and_then(|map| map.get(type_id))
        .filter(|value| value.is_object())
}

fn build_action_request(payload: &Value) -> Map<String, Value> {
    let Some(payload_map) = payload.as_object() else {
        return Map::new();
    };
    if let Some(request) = payload_map.get("request").and_then(Value::as_object) {
        return request.clone();
    }
    payload_map.clone()
}

fn action_payload_type_id(payload: &Value) -> Option<&str> {
    payload
        .as_object()
        .and_then(|obj| obj.get("type_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn extract_track_token_from_payload(
    payload: &Value,
    action: &str,
) -> Result<String, (StatusCode, String)> {
    let request = build_action_request(payload);
    if let Some(token) = request
        .get("track_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(token.to_string());
    }

    let Some(track_ref) = request.get("track_ref").or_else(|| request.get("track")) else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("host action `{action}` requires payload.track_ref or payload.track_token"),
        ));
    };

    if let Some(token) = track_ref
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(token.to_string());
    }

    let Some(track_obj) = track_ref.as_object() else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "host action `{action}` expects `track_ref` as string token or object {{source_id, track_id, locator}}"
            ),
        ));
    };

    let source_id = track_obj
        .get("source_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("host action `{action}` track_ref.source_id is required"),
            )
        })?;
    let track_id = track_obj
        .get("track_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("host action `{action}` track_ref.track_id is required"),
            )
        })?;
    let locator = track_obj
        .get("locator")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("host action `{action}` track_ref.locator is required"),
            )
        })?;

    if !source_id.eq_ignore_ascii_case("local") {
        validate_source_locator_json(locator, action)?;
    }

    serde_json::to_string(&json!({
        "source_id": source_id,
        "track_id": track_id,
        "locator": locator,
    }))
    .map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode track_ref token: {error}"),
        )
    })
}

fn validate_source_locator_json(locator: &str, action: &str) -> Result<(), (StatusCode, String)> {
    let parsed = serde_json::from_str::<Value>(locator).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!(
                "host action `{action}` track_ref.locator must be source locator JSON: {error}"
            ),
        )
    })?;
    let Some(obj) = parsed.as_object() else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("host action `{action}` track_ref.locator must be a JSON object"),
        ));
    };
    for required in ["plugin_id", "type_id", "config", "track"] {
        if !obj.contains_key(required) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "host action `{action}` track_ref.locator missing required field `{required}`"
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_action_request_prefers_request_object() {
        let payload = json!({
            "request": {
                "keywords": "test",
                "limit": 10
            },
            "ignored": true
        });
        let request = build_action_request(&payload);
        assert_eq!(
            request.get("keywords"),
            Some(&Value::String("test".to_string()))
        );
        assert_eq!(request.get("limit"), Some(&Value::Number(10.into())));
        assert_eq!(request.get("ignored"), None);
    }

    #[test]
    fn extract_track_token_accepts_direct_token() {
        let payload = json!({
            "request": {
                "track_token": "token-123"
            }
        });
        let token = extract_track_token_from_payload(&payload, "playback.play_track_ref")
            .expect("direct token should parse");
        assert_eq!(token, "token-123");
    }

    #[test]
    fn extract_track_token_accepts_track_ref_object() {
        let locator = json!({
            "plugin_id": "dev.stellatune.source.netease",
            "type_id": "netease",
            "config": {},
            "track": { "song_id": 33894312, "level": "standard" },
            "ext_hint": "mp3",
            "path_hint": "netease:33894312.mp3"
        })
        .to_string();
        let payload = json!({
            "track_ref": {
                "source_id": "netease",
                "track_id": "33894312",
                "locator": locator
            }
        });
        let token = extract_track_token_from_payload(&payload, "playback.play_track_ref")
            .expect("track_ref object should encode token");
        let decoded: Value =
            serde_json::from_str(token.as_str()).expect("token should be serialized JSON");
        assert_eq!(decoded["source_id"], "netease");
        assert_eq!(decoded["track_id"], "33894312");
        assert!(decoded["locator"].as_str().is_some());
    }

    #[test]
    fn extract_track_token_rejects_missing_locator() {
        let payload = json!({
            "request": {
                "track_ref": {
                    "source_id": "netease",
                    "track_id": "33894312"
                }
            }
        });
        let error = extract_track_token_from_payload(&payload, "playback.play_track_ref")
            .expect_err("missing locator should fail");
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(error.1.contains("track_ref.locator is required"));
    }

    #[test]
    fn extract_track_token_rejects_non_json_locator_for_source_track() {
        let payload = json!({
            "track_ref": {
                "source_id": "netease",
                "track_id": "33894312",
                "locator": "netease:33894312.mp3"
            }
        });
        let error = extract_track_token_from_payload(&payload, "playback.play_track_ref")
            .expect_err("source locator shorthand should fail early");
        assert_eq!(error.0, StatusCode::BAD_REQUEST);
        assert!(
            error
                .1
                .contains("track_ref.locator must be source locator JSON")
        );
    }
}
