use std::collections::BTreeSet;

use axum::http::StatusCode;
use serde_json::{Map, Value, json};
use stellatune_plugins::typescript::manifest::TypeScriptCapabilityKind;

use crate::plugin_ui_gateway::model::{ConfigApplyOutcome, ConfigApplyReport};
use crate::runtime::{shared_runtime_engine, shared_typescript_runtime};

const ACTION_PLAYBACK_PLAY_TRACK_REF: &str = "playback.play_track_ref";
const ACTION_PLAYBACK_ENQUEUE_TRACK_REF: &str = "playback.enqueue_track_ref";
const ACTION_PLAYBACK_PAUSE: &str = "playback.pause";
const ACTION_PLAYBACK_NEXT: &str = "playback.next";
const ACTION_PLAYBACK_STOP: &str = "playback.stop";

pub(super) fn validate_config_payload(config: &Value) -> Result<(), String> {
    let Some(root) = config.as_object() else {
        return Err("config payload must be a JSON object".to_string());
    };
    for (group, values) in root {
        let Some(values) = values.as_object() else {
            return Err(format!("config group `{group}` must be an object"));
        };
        for (capability_id, value) in values {
            if capability_id.trim().is_empty() || !value.is_object() {
                return Err(format!(
                    "config `{group}/{capability_id}` must be a JSON object"
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
    let registrations = shared_typescript_runtime().registered_plugins().await;
    let registration = registrations
        .iter()
        .find(|registration| registration.manifest.id == plugin_id);
    let mut configured = BTreeSet::new();
    for (group, values) in config.as_object().into_iter().flatten() {
        if let Some(values) = values.as_object() {
            for capability_id in values.keys() {
                configured.insert((group.clone(), capability_id.clone()));
            }
        }
    }
    let mut outcomes = Vec::new();
    for (group, capability_id) in configured {
        let known = registration.is_some_and(|registration| {
            registration
                .manifest
                .capabilities
                .iter()
                .any(|capability| capability.id == capability_id)
        });
        outcomes.push(ConfigApplyOutcome {
            kind: group,
            type_id: capability_id,
            status: if known { "applied" } else { "skipped" }.to_string(),
            detail: Some(if known {
                "configuration will be supplied with each capability invocation".to_string()
            } else {
                "capability is not registered".to_string()
            }),
        });
    }
    let applied = outcomes
        .iter()
        .filter(|item| item.status == "applied")
        .count();
    let skipped = outcomes.len().saturating_sub(applied);
    Ok(ConfigApplyReport {
        plugin_id: plugin_id.to_string(),
        applied,
        skipped,
        failed: 0,
        outcomes,
    })
}

pub(super) fn action_payload_config(payload: &Value) -> Option<&Value> {
    payload
        .as_object()?
        .get("config")
        .filter(|value| value.is_object())
}

pub(super) fn action_payload_persist(payload: &Value) -> bool {
    payload
        .as_object()
        .and_then(|object| object.get("persist"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

pub(super) fn build_apply_report_json(report: &ConfigApplyReport) -> Value {
    serde_json::to_value(report).unwrap_or_else(|_| json!({ "plugin_id": report.plugin_id }))
}

pub(super) fn normalize_action_payload(payload: Value) -> Value {
    if payload.is_null() {
        Value::Object(Map::new())
    } else {
        payload
    }
}

pub(super) fn build_unknown_action_error(action: &str) -> String {
    format!("unsupported action `{action}`")
}

pub(super) async fn invoke_action_via_host(
    action: &str,
    payload: &Value,
) -> Result<Option<Value>, (StatusCode, String)> {
    let engine = shared_runtime_engine();
    match action.trim() {
        ACTION_PLAYBACK_PLAY_TRACK_REF | ACTION_PLAYBACK_NEXT => {
            let token = extract_track_token(payload, action)?;
            engine
                .switch_track_token(token.clone(), true)
                .await
                .map_err(internal)?;
            Ok(Some(
                json!({ "dispatch": "host.playback", "track_token": token }),
            ))
        },
        ACTION_PLAYBACK_ENQUEUE_TRACK_REF => {
            let token = extract_track_token(payload, action)?;
            engine
                .queue_next_track_token(token.clone())
                .await
                .map_err(internal)?;
            Ok(Some(
                json!({ "dispatch": "host.playback", "track_token": token, "queued": true }),
            ))
        },
        ACTION_PLAYBACK_PAUSE => {
            engine.pause().await.map_err(internal)?;
            Ok(Some(json!({ "dispatch": "host.playback", "paused": true })))
        },
        ACTION_PLAYBACK_STOP => {
            engine.stop().await.map_err(internal)?;
            Ok(Some(
                json!({ "dispatch": "host.playback", "stopped": true }),
            ))
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
    let runtime = shared_typescript_runtime();
    let registrations = runtime.registered_plugins().await;
    let Some(registration) = registrations
        .iter()
        .find(|registration| registration.manifest.id == plugin_id)
    else {
        return Ok(None);
    };
    let requested = payload
        .get("capabilityId")
        .or_else(|| payload.get("type_id"))
        .and_then(Value::as_str);
    let preferred_kind = if action.contains("auth") {
        TypeScriptCapabilityKind::AuthProvider
    } else if action.contains("lyric") {
        TypeScriptCapabilityKind::LyricsProvider
    } else {
        TypeScriptCapabilityKind::NetworkControl
    };
    let capability = requested
        .and_then(|id| {
            registration
                .manifest
                .capabilities
                .iter()
                .find(|item| item.id == id)
        })
        .or_else(|| {
            registration
                .manifest
                .capabilities
                .iter()
                .find(|item| item.kind == preferred_kind)
        });
    let Some(capability) = capability else {
        return Ok(None);
    };
    let mut input = payload.as_object().cloned().unwrap_or_default();
    input.insert("action".to_string(), Value::String(action.to_string()));
    if let Some(config) = find_capability_config(config_root, &capability.id) {
        input.insert("config".to_string(), config.clone());
    }
    let result = runtime
        .invoke(
            plugin_id,
            &capability.id,
            None,
            "invoke",
            Value::Object(input),
            None,
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(Some(json!({
        "dispatch": "typescript.capability",
        "capability_id": capability.id,
        "response": result.value,
    })))
}

fn find_capability_config<'a>(root: &'a Value, capability_id: &str) -> Option<&'a Value> {
    root.as_object()?
        .values()
        .filter_map(Value::as_object)
        .find_map(|group| group.get(capability_id))
}

fn extract_track_token(payload: &Value, action: &str) -> Result<String, (StatusCode, String)> {
    payload
        .get("track_token")
        .or_else(|| payload.get("trackToken"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("host action `{action}` requires track_token"),
            )
        })
}

fn internal(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
