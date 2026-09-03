use std::collections::BTreeSet;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Map, Value, json};
use stellatune_plugins::typescript::manifest::TypeScriptCapabilityKind;

use crate::player_service::identity::{
    ProviderId, ProviderTrackIdentityInput, ProviderTrackKeyInput, TrackId,
};
use crate::player_service::source::SourceResolverSpec;
use crate::plugin_ui_gateway::model::{ConfigApplyOutcome, ConfigApplyReport};
use crate::runtime::{
    TypeScriptSourceResolver, shared_playback_controller, shared_player_service,
    shared_typescript_runtime,
};
use stellatune_audio::playback::control::SwitchOptions;

const ACTION_PLAYBACK_PLAY_TRACK: &str = "playback.play_track";
const ACTION_PLAYBACK_ENQUEUE_TRACK: &str = "playback.enqueue_track";
const ACTION_PLAYBACK_PLAY_PROVIDER_TRACK: &str = "playback.play_provider_track";
const ACTION_PLAYBACK_ENQUEUE_PROVIDER_TRACK: &str = "playback.enqueue_provider_track";
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
    plugin_id: &str,
    action: &str,
    payload: &Value,
    config_root: &Value,
) -> Result<Option<Value>, (StatusCode, String)> {
    let player = shared_playback_controller();
    match action.trim() {
        ACTION_PLAYBACK_PLAY_TRACK | ACTION_PLAYBACK_NEXT => {
            let track_id = extract_track_id(payload, action)?;
            let service = shared_player_service().ok_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "player service is not initialized".to_owned(),
                )
            })?;
            service
                .switch_track(track_id, SwitchOptions::default())
                .await
                .map_err(internal)?;
            Ok(Some(
                json!({ "dispatch": "host.playback", "track_id": track_id.get() }),
            ))
        },
        ACTION_PLAYBACK_ENQUEUE_TRACK => {
            let track_id = extract_track_id(payload, action)?;
            let service = shared_player_service().ok_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "player service is not initialized".to_owned(),
                )
            })?;
            service.queue_next(track_id).await.map_err(internal)?;
            Ok(Some(
                json!({ "dispatch": "host.playback", "track_id": track_id.get(), "queued": true }),
            ))
        },
        ACTION_PLAYBACK_PLAY_PROVIDER_TRACK | ACTION_PLAYBACK_ENQUEUE_PROVIDER_TRACK => {
            let track_id = ensure_provider_track(plugin_id, payload, config_root).await?;
            let service = shared_player_service().ok_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "player service is not initialized".to_owned(),
                )
            })?;
            if action.trim() == ACTION_PLAYBACK_PLAY_PROVIDER_TRACK {
                service
                    .switch_track(track_id, SwitchOptions::default())
                    .await
                    .map_err(internal)?;
                Ok(Some(json!({
                    "dispatch": "host.playback",
                    "track_id": track_id.get(),
                    "autoplay": true,
                })))
            } else {
                service.queue_next(track_id).await.map_err(internal)?;
                Ok(Some(json!({
                    "dispatch": "host.playback",
                    "track_id": track_id.get(),
                    "queued": true,
                })))
            }
        },
        ACTION_PLAYBACK_PAUSE => {
            player.pause().await.map_err(internal)?;
            Ok(Some(json!({ "dispatch": "host.playback", "paused": true })))
        },
        ACTION_PLAYBACK_STOP => {
            player.stop().await.map_err(internal)?;
            Ok(Some(
                json!({ "dispatch": "host.playback", "stopped": true }),
            ))
        },
        _ => Ok(None),
    }
}

async fn ensure_provider_track(
    plugin_id: &str,
    payload: &Value,
    config_root: &Value,
) -> Result<TrackId, (StatusCode, String)> {
    let object = payload
        .as_object()
        .ok_or_else(|| bad_request("provider playback payload must be an object"))?;
    let source_type_id = object
        .get("source_type_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| bad_request("source_type_id is required"))?;
    let provider_id = object
        .get("provider_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(source_type_id);
    let provider_key = object
        .get("provider_track_key")
        .ok_or_else(|| bad_request("provider_track_key is required"))?;
    let provider_key = if let Some(value) = provider_key.as_u64() {
        ProviderTrackKeyInput::Numeric(value)
    } else if let Some(value) = provider_key
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        value
            .parse::<u64>()
            .ok()
            .filter(|numeric| numeric.to_string() == value)
            .map(ProviderTrackKeyInput::Numeric)
            .unwrap_or_else(|| ProviderTrackKeyInput::Text(value.to_owned()))
    } else {
        return Err(bad_request(
            "provider_track_key must be a positive integer or non-empty string",
        ));
    };
    let config = find_capability_config(config_root, source_type_id)
        .cloned()
        .unwrap_or_else(|| json!({}));
    let resolver = Arc::new(
        TypeScriptSourceResolver::new(
            shared_typescript_runtime(),
            plugin_id,
            source_type_id,
            config.clone(),
        )
        .map_err(internal)?,
    );
    let resolver_spec = SourceResolverSpec::new(
        plugin_id,
        source_type_id,
        serde_json::to_string(&config).map_err(internal)?,
    )
    .map_err(internal)?;
    let binding = ProviderId::new(format!("{plugin_id}::{source_type_id}::{provider_id}"))
        .map_err(bad_request)?;
    let service = shared_player_service().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "player service is not initialized".to_owned(),
        )
    })?;
    let source = service
        .ensure_plugin_source(binding, resolver_spec, resolver)
        .await
        .map_err(internal)?;
    service
        .ensure_track(ProviderTrackIdentityInput {
            source_instance_id: source.get(),
            provider_key,
        })
        .await
        .map_err(internal)
}

fn bad_request(message: impl ToString) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, message.to_string())
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

fn extract_track_id(payload: &Value, action: &str) -> Result<TrackId, (StatusCode, String)> {
    let raw = payload
        .get("track_id")
        .or_else(|| payload.get("trackId"))
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("host action `{action}` requires track_id"),
            )
        })?;
    TrackId::new(raw).map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))
}

fn internal(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
