use core::ffi::c_void;

use serde_json::{Map, Value};
use stellatune_core::{AudioBackend, OutputSinkRoute, PlayerControlCommand};

use crate::*;

#[derive(Default)]
struct DummyOutputSink;

impl OutputSink for DummyOutputSink {
    fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> Result<u32, String> {
        if channels == 0 {
            return Err("invalid channels".to_string());
        }
        if !samples.len().is_multiple_of(channels as usize) {
            return Err("unaligned samples".to_string());
        }
        Ok((samples.len() / channels as usize) as u32)
    }
}

impl OutputSinkDescriptor for DummyOutputSink {
    const TYPE_ID: &'static str = "dummy.output";
    const DISPLAY_NAME: &'static str = "Dummy Output";
    const CONFIG_SCHEMA_JSON: &'static str = "{}";
    const DEFAULT_CONFIG_JSON: &'static str = "{}";

    fn list_targets_json(_config_json: &str) -> Result<String, String> {
        Ok(r#"[{"id":"dummy-0","name":"Dummy"}]"#.to_string())
    }

    fn open(_spec: StAudioSpec, _config_json: &str, _target_json: &str) -> Result<Self, String> {
        Ok(Self)
    }
}

crate::export_output_sink_interface! {
    sink: DummyOutputSink,
}

fn ststr_ref(s: &str) -> StStr {
    StStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

#[test]
fn build_player_control_request_json_includes_scope_command_and_fields() {
    let mut fields = Map::new();
    fields.insert("position_ms".to_string(), Value::from(12345u64));
    let raw = build_player_control_request_json(
        PlayerControlCommand::SeekMs,
        Some(Value::String("req-1".to_string())),
        Some(fields),
    )
    .expect("build request");
    let v: Value = serde_json::from_str(&raw).expect("parse request");
    assert_eq!(v["scope"], Value::String("player".to_string()));
    assert_eq!(v["command"], Value::String("seek_ms".to_string()));
    assert_eq!(v["request_id"], Value::String("req-1".to_string()));
    assert_eq!(v["position_ms"], Value::from(12345u64));
}

#[test]
fn parse_control_ack_json_works() {
    let ack = parse_control_ack_json(r#"{"ok":true}"#).expect("parse ack");
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
            assert_eq!(payload.request_id, Some(Value::String("req-1".to_string())));
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
    match request_id {
        Value::String(v) => assert!(v.starts_with("req-")),
        other => panic!("unexpected request id: {other:?}"),
    }
}

#[test]
fn control_event_helpers_match_request_id() {
    let raw = r#"{"topic":"host.control.finished","request_id":"req-9","scope":"player","command":"play","ok":true}"#;
    let event = parse_host_event_json(raw).expect("parse event");
    assert!(as_control_finished(&event).is_some());
    assert!(as_control_result(&event).is_none());
    assert!(control_event_matches_request_id(
        &event,
        &Value::String("req-9".to_string())
    ));
    assert!(!control_event_matches_request_id(
        &event,
        &Value::String("req-other".to_string())
    ));
}

#[test]
fn player_fields_output_helpers_build_expected_payload() {
    let route = OutputSinkRoute {
        plugin_id: "dev.stellatune.output.asio".to_string(),
        type_id: "asio".to_string(),
        config_json: r#"{"buffer_ms":10}"#.to_string(),
        target_json: r#"{"id":"asio-device-1"}"#.to_string(),
    };
    let route_fields = player_fields_set_output_sink_route(&route).expect("route fields");
    assert_eq!(
        route_fields["route"]["plugin_id"],
        Value::String(route.plugin_id)
    );
    assert_eq!(
        route_fields["route"]["type_id"],
        Value::String(route.type_id)
    );

    let device_fields =
        player_fields_set_output_device(AudioBackend::Shared, Some("device-1".to_string()))
            .expect("device fields");
    assert_eq!(
        device_fields["backend"],
        Value::String("Shared".to_string())
    );
    assert_eq!(
        device_fields["device_id"],
        Value::String("device-1".to_string())
    );

    let options_fields = player_fields_set_output_options(true, false, true);
    assert_eq!(options_fields["match_track_sample_rate"], Value::Bool(true));
    assert_eq!(options_fields["gapless_playback"], Value::Bool(false));
    assert_eq!(options_fields["seek_track_fade"], Value::Bool(true));
}

#[test]
fn exported_output_sink_interface_works() {
    let iface = __st_output_sink_get_interface(ststr(ST_INTERFACE_OUTPUT_SINK_V1));
    assert!(!iface.is_null());
    let vt = iface as *const StOutputSinkVTableV1;
    assert!(!vt.is_null());

    let cfg = ststr_ref("{}");
    let mut out_json = StStr::empty();
    let st = unsafe { ((*vt).list_targets_json_utf8)(cfg, &mut out_json) };
    assert_eq!(st.code, 0);
    let targets = unsafe { ststr_to_str(&out_json) }.expect("targets utf8");
    assert!(targets.contains("dummy-0"));
    plugin_free(out_json.ptr as *mut c_void, out_json.len, 1);

    let target = ststr_ref(r#"{"id":"dummy-0"}"#);
    let spec = StAudioSpec {
        sample_rate: 48_000,
        channels: 2,
        reserved: 0,
    };
    let mut handle: *mut c_void = core::ptr::null_mut();
    let st = unsafe { ((*vt).open)(cfg, target, spec, &mut handle) };
    assert_eq!(st.code, 0);
    assert!(!handle.is_null());

    let samples = [0.1_f32, -0.1_f32, 0.2_f32, -0.2_f32];
    let mut accepted = 0u32;
    let st =
        unsafe { ((*vt).write_interleaved_f32)(handle, 2, 2, samples.as_ptr(), &mut accepted) };
    assert_eq!(st.code, 0);
    assert_eq!(accepted, 2);

    let flush = unsafe { (*vt).flush }.expect("flush callback");
    let st = flush(handle);
    assert_eq!(st.code, 0);

    unsafe { ((*vt).close)(handle) };
}
