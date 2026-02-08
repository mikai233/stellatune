use core::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};
use stellatune_core::{
    AudioBackend, ControlScope, HostControlFinishedPayload, HostControlResultPayload,
    HostEventTopic, HostLibraryEventEnvelope, HostPlayerEventEnvelope, HostPlayerTickPayload,
    LibraryControlCommand, OutputSinkRoute, PlayerControlCommand,
};

use crate::{host_poll_event_json, host_send_control_json};

static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct HostControlAck {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginHostEvent {
    PlayerTick(HostPlayerTickPayload),
    PlayerEvent(HostPlayerEventEnvelope),
    LibraryEvent(HostLibraryEventEnvelope),
    ControlResult(HostControlResultPayload),
    ControlFinished(HostControlFinishedPayload),
    Custom(Value),
}

pub fn next_request_id() -> Value {
    let seq = REQUEST_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Value::String(format!("req-{ts_ms}-{seq}"))
}

pub fn as_control_result(event: &PluginHostEvent) -> Option<&HostControlResultPayload> {
    match event {
        PluginHostEvent::ControlResult(payload) => Some(payload),
        _ => None,
    }
}

pub fn as_control_finished(event: &PluginHostEvent) -> Option<&HostControlFinishedPayload> {
    match event {
        PluginHostEvent::ControlFinished(payload) => Some(payload),
        _ => None,
    }
}

pub fn control_event_request_id(event: &PluginHostEvent) -> Option<&Value> {
    match event {
        PluginHostEvent::ControlResult(payload) => payload.request_id.as_ref(),
        PluginHostEvent::ControlFinished(payload) => payload.request_id.as_ref(),
        _ => None,
    }
}

pub fn control_event_matches_request_id(event: &PluginHostEvent, request_id: &Value) -> bool {
    matches!(control_event_request_id(event), Some(v) if v == request_id)
}

fn build_control_request_json(
    scope: ControlScope,
    command: &str,
    request_id: Option<Value>,
    fields: Option<Map<String, Value>>,
) -> Result<String, String> {
    let mut root = fields.unwrap_or_default();
    root.insert(
        "scope".to_string(),
        Value::String(scope.as_str().to_string()),
    );
    root.insert("command".to_string(), Value::String(command.to_string()));
    if let Some(request_id) = request_id {
        root.insert("request_id".to_string(), request_id);
    }
    serde_json::to_string(&Value::Object(root)).map_err(|e| e.to_string())
}

pub fn build_player_control_request_json(
    command: PlayerControlCommand,
    request_id: Option<Value>,
    fields: Option<Map<String, Value>>,
) -> Result<String, String> {
    build_control_request_json(ControlScope::Player, command.as_str(), request_id, fields)
}

pub fn build_library_control_request_json(
    command: LibraryControlCommand,
    request_id: Option<Value>,
    fields: Option<Map<String, Value>>,
) -> Result<String, String> {
    build_control_request_json(ControlScope::Library, command.as_str(), request_id, fields)
}

pub fn parse_control_ack_json(response_json: &str) -> Result<HostControlAck, String> {
    serde_json::from_str(response_json).map_err(|e| e.to_string())
}

pub fn host_send_player_control(
    command: PlayerControlCommand,
    request_id: Option<Value>,
    fields: Option<Map<String, Value>>,
) -> Result<HostControlAck, String> {
    let request = build_player_control_request_json(command, request_id, fields)?;
    let response = host_send_control_json(&request)?;
    parse_control_ack_json(&response)
}

pub fn host_send_library_control(
    command: LibraryControlCommand,
    request_id: Option<Value>,
    fields: Option<Map<String, Value>>,
) -> Result<HostControlAck, String> {
    let request = build_library_control_request_json(command, request_id, fields)?;
    let response = host_send_control_json(&request)?;
    parse_control_ack_json(&response)
}

pub fn player_fields_set_output_sink_route(
    route: &OutputSinkRoute,
) -> Result<Map<String, Value>, String> {
    let mut fields = Map::new();
    let route_value = serde_json::to_value(route).map_err(|e| e.to_string())?;
    fields.insert("route".to_string(), route_value);
    Ok(fields)
}

pub fn player_fields_set_output_device(
    backend: AudioBackend,
    device_id: Option<String>,
) -> Result<Map<String, Value>, String> {
    let mut fields = Map::new();
    let backend_value = serde_json::to_value(backend).map_err(|e| e.to_string())?;
    fields.insert("backend".to_string(), backend_value);
    fields.insert(
        "device_id".to_string(),
        device_id.map_or(Value::Null, Value::String),
    );
    Ok(fields)
}

pub fn player_fields_set_output_options(
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(
        "match_track_sample_rate".to_string(),
        Value::Bool(match_track_sample_rate),
    );
    fields.insert(
        "gapless_playback".to_string(),
        Value::Bool(gapless_playback),
    );
    fields.insert("seek_track_fade".to_string(), Value::Bool(seek_track_fade));
    fields
}

pub fn host_set_output_sink_route(
    route: &OutputSinkRoute,
    request_id: Option<Value>,
) -> Result<HostControlAck, String> {
    host_send_player_control(
        PlayerControlCommand::SetOutputSinkRoute,
        request_id,
        Some(player_fields_set_output_sink_route(route)?),
    )
}

pub fn host_clear_output_sink_route(request_id: Option<Value>) -> Result<HostControlAck, String> {
    host_send_player_control(PlayerControlCommand::ClearOutputSinkRoute, request_id, None)
}

pub fn host_set_output_device(
    backend: AudioBackend,
    device_id: Option<String>,
    request_id: Option<Value>,
) -> Result<HostControlAck, String> {
    host_send_player_control(
        PlayerControlCommand::SetOutputDevice,
        request_id,
        Some(player_fields_set_output_device(backend, device_id)?),
    )
}

pub fn host_set_output_options(
    match_track_sample_rate: bool,
    gapless_playback: bool,
    seek_track_fade: bool,
    request_id: Option<Value>,
) -> Result<HostControlAck, String> {
    host_send_player_control(
        PlayerControlCommand::SetOutputOptions,
        request_id,
        Some(player_fields_set_output_options(
            match_track_sample_rate,
            gapless_playback,
            seek_track_fade,
        )),
    )
}

pub fn host_refresh_devices(request_id: Option<Value>) -> Result<HostControlAck, String> {
    host_send_player_control(PlayerControlCommand::RefreshDevices, request_id, None)
}

pub fn parse_host_event_json(event_json: &str) -> Result<PluginHostEvent, String> {
    let root: Value = serde_json::from_str(event_json).map_err(|e| e.to_string())?;
    let topic = root
        .get("topic")
        .and_then(|v| v.as_str())
        .and_then(HostEventTopic::from_str);
    let Some(topic) = topic else {
        return Ok(PluginHostEvent::Custom(root));
    };
    match topic {
        HostEventTopic::PlayerTick => serde_json::from_value::<HostPlayerTickPayload>(root)
            .map(PluginHostEvent::PlayerTick)
            .map_err(|e| e.to_string()),
        HostEventTopic::PlayerEvent => serde_json::from_value::<HostPlayerEventEnvelope>(root)
            .map(PluginHostEvent::PlayerEvent)
            .map_err(|e| e.to_string()),
        HostEventTopic::LibraryEvent => serde_json::from_value::<HostLibraryEventEnvelope>(root)
            .map(PluginHostEvent::LibraryEvent)
            .map_err(|e| e.to_string()),
        HostEventTopic::HostControlResult => {
            serde_json::from_value::<HostControlResultPayload>(root)
                .map(PluginHostEvent::ControlResult)
                .map_err(|e| e.to_string())
        }
        HostEventTopic::HostControlFinished => {
            serde_json::from_value::<HostControlFinishedPayload>(root)
                .map(PluginHostEvent::ControlFinished)
                .map_err(|e| e.to_string())
        }
    }
}

pub fn host_poll_event() -> Result<Option<PluginHostEvent>, String> {
    let Some(raw) = host_poll_event_json()? else {
        return Ok(None);
    };
    parse_host_event_json(&raw).map(Some)
}
