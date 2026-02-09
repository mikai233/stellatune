use serde_json::Value;
use stellatune_core::{AudioBackend, OutputSinkRoute};

use crate::*;

#[test]
fn player_control_to_request_json_includes_scope_command_and_fields() {
    let raw = PlayerControl::seek_ms(12345)
        .to_request_json(Some(RequestId::new("req-1")))
        .expect("build request");
    let v: Value = serde_json::from_str(&raw).expect("parse request");
    assert_eq!(v["scope"], Value::String("player".to_string()));
    assert_eq!(v["command"], Value::String("seek_ms".to_string()));
    assert_eq!(v["request_id"], Value::String("req-1".to_string()));
    assert_eq!(v["position_ms"], Value::from(12345u64));
}

#[test]
fn player_control_typed_builder_builds_expected_json() {
    let raw = PlayerControl::seek_ms(42)
        .to_request_json(Some(RequestId::new("req-typed")))
        .expect("build typed request");
    let v: Value = serde_json::from_str(&raw).expect("parse typed request");
    assert_eq!(v["scope"], Value::String("player".to_string()));
    assert_eq!(v["command"], Value::String("seek_ms".to_string()));
    assert_eq!(v["request_id"], Value::String("req-typed".to_string()));
    assert_eq!(v["position_ms"], Value::from(42u64));
}

#[test]
fn library_control_typed_builder_builds_expected_json() {
    let query = LibraryListTracksQuery::new()
        .folder("C:/Music")
        .recursive(false)
        .query("radiohead")
        .limit(100)
        .offset(20);
    let raw = LibraryControl::list_tracks(query)
        .to_request_json(Some(RequestId::new("req-lib-typed")))
        .expect("build typed library request");
    let v: Value = serde_json::from_str(&raw).expect("parse typed library request");
    assert_eq!(v["scope"], Value::String("library".to_string()));
    assert_eq!(v["command"], Value::String("list_tracks".to_string()));
    assert_eq!(v["request_id"], Value::String("req-lib-typed".to_string()));
    assert_eq!(v["folder"], Value::String("C:/Music".to_string()));
    assert_eq!(v["recursive"], Value::Bool(false));
    assert_eq!(v["query"], Value::String("radiohead".to_string()));
    assert_eq!(v["limit"], Value::from(100i64));
    assert_eq!(v["offset"], Value::from(20i64));
}

#[test]
fn control_ack_deserialize_works() {
    let ack: HostControlAck = serde_json::from_str(r#"{"ok":true}"#).expect("parse ack");
    assert_eq!(
        ack,
        HostControlAck {
            ok: true,
            error: None
        }
    );
}

#[test]
fn parse_host_event_json_recognizes_control_result() {
    let raw = r#"{"topic":"host.control.result","request_id":"req-1","scope":"player","command":"play","ok":true}"#;
    let event = parse_host_event_json(raw).expect("parse event");
    match event {
        PluginHostEvent::ControlResult(payload) => {
            assert!(payload.ok);
            assert_eq!(payload.request_id, Some(RequestId::new("req-1")));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn parse_host_event_json_falls_back_to_custom_when_topic_missing() {
    let raw = r#"{"hello":"world"}"#;
    let event = parse_host_event_json(raw).expect("parse custom");
    match event {
        PluginHostEvent::Custom(v) => {
            assert_eq!(v["hello"], Value::String("world".to_string()));
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn next_request_id_returns_string_value() {
    let request_id = next_request_id();
    assert!(request_id.as_str().starts_with("req-"));
}

#[test]
fn control_event_helpers_match_request_id() {
    let raw = r#"{"topic":"host.control.finished","request_id":"req-9","scope":"player","command":"play","ok":true}"#;
    let event = parse_host_event_json(raw).expect("parse event");
    assert!(as_control_finished(&event).is_some());
    assert!(as_control_result(&event).is_none());
    assert!(control_event_matches_request_id(
        &event,
        &RequestId::new("req-9")
    ));
    assert!(!control_event_matches_request_id(
        &event,
        &RequestId::new("req-other")
    ));
}

#[test]
fn player_typed_controls_include_expected_payload_fields() {
    let route = OutputSinkRoute {
        plugin_id: "dev.stellatune.output.asio".to_string(),
        type_id: "asio".to_string(),
        config_json: r#"{"buffer_ms":10}"#.to_string(),
        target_json: r#"{"id":"asio-device-1"}"#.to_string(),
    };
    let route_raw = PlayerControl::set_output_sink_route(route.clone())
        .to_request_json(None)
        .expect("route request");
    let route_json: Value = serde_json::from_str(&route_raw).expect("parse route request");
    assert_eq!(
        route_json["route"]["plugin_id"],
        Value::String(route.plugin_id)
    );
    assert_eq!(route_json["route"]["type_id"], Value::String(route.type_id));

    let device_raw =
        PlayerControl::set_output_device(AudioBackend::Shared, Some("device-1".to_string()))
            .to_request_json(None)
            .expect("device request");
    let device_json: Value = serde_json::from_str(&device_raw).expect("parse device request");
    assert_eq!(device_json["backend"], Value::String("Shared".to_string()));
    assert_eq!(
        device_json["device_id"],
        Value::String("device-1".to_string())
    );

    let options_raw = PlayerControl::set_output_options(true, false, true)
        .to_request_json(None)
        .expect("options request");
    let options_json: Value = serde_json::from_str(&options_raw).expect("parse options request");
    assert_eq!(options_json["match_track_sample_rate"], Value::Bool(true));
    assert_eq!(options_json["gapless_playback"], Value::Bool(false));
    assert_eq!(options_json["seek_track_fade"], Value::Bool(true));
}
