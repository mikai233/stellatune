use core::ffi::c_void;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use stellatune_core::{AudioBackend, OutputSinkRoute};

use crate::*;

#[derive(Default)]
struct DummyOutputSink;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DummyOutputConfig {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DummyOutputTarget {
    id: String,
}

impl OutputSink for DummyOutputSink {
    fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> SdkResult<u32> {
        if channels == 0 {
            return Err(SdkError::msg("invalid channels"));
        }
        if !samples.len().is_multiple_of(channels as usize) {
            return Err(SdkError::msg("unaligned samples"));
        }
        Ok((samples.len() / channels as usize) as u32)
    }
}

impl OutputSinkDescriptor for DummyOutputSink {
    type Config = DummyOutputConfig;
    type Target = DummyOutputTarget;

    const TYPE_ID: &'static str = "dummy.output";
    const DISPLAY_NAME: &'static str = "Dummy Output";
    const CONFIG_SCHEMA_JSON: &'static str = "{}";

    fn default_config() -> Self::Config {
        DummyOutputConfig::default()
    }

    fn list_targets(_config: &Self::Config) -> SdkResult<Vec<Self::Target>> {
        Ok(vec![DummyOutputTarget {
            id: "dummy-0".to_string(),
        }])
    }

    fn open(_spec: StAudioSpec, _config: &Self::Config, _target: &Self::Target) -> SdkResult<Self> {
        Ok(Self)
    }
}

#[derive(Default)]
struct DummySourceStream {
    data: Vec<u8>,
    cursor: usize,
}

impl DummySourceStream {
    fn with_bytes(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
            cursor: 0,
        }
    }
}

impl SourceStream for DummySourceStream {
    const SUPPORTS_SEEK: bool = true;
    const SUPPORTS_TELL: bool = true;
    const SUPPORTS_SIZE: bool = true;

    fn read(&mut self, out: &mut [u8]) -> SdkResult<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        let remain = self.data.len().saturating_sub(self.cursor);
        let n = remain.min(out.len());
        out[..n].copy_from_slice(&self.data[self.cursor..self.cursor + n]);
        self.cursor += n;
        Ok(n)
    }

    fn seek(&mut self, offset: i64, whence: StSeekWhence) -> SdkResult<u64> {
        let base = match whence {
            StSeekWhence::Start => 0i64,
            StSeekWhence::Current => self.cursor as i64,
            StSeekWhence::End => self.data.len() as i64,
        };
        let next = base
            .checked_add(offset)
            .ok_or_else(|| SdkError::msg("seek overflow"))?;
        if next < 0 {
            return Err(SdkError::msg("seek before start"));
        }
        let next = next as usize;
        if next > self.data.len() {
            return Err(SdkError::msg("seek past end"));
        }
        self.cursor = next;
        Ok(self.cursor as u64)
    }

    fn tell(&mut self) -> SdkResult<u64> {
        Ok(self.cursor as u64)
    }

    fn size(&mut self) -> SdkResult<u64> {
        Ok(self.data.len() as u64)
    }
}

struct DummySourceCatalog;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DummySourceConfig {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DummyListRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DummyListItem {
    id: String,
    title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DummyTrackRef {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DummyTrackMeta {
    title: String,
}

impl SourceCatalogDescriptor for DummySourceCatalog {
    type Stream = DummySourceStream;
    type Config = DummySourceConfig;
    type ListRequest = DummyListRequest;
    type ListItem = DummyListItem;
    type Track = DummyTrackRef;
    type TrackMeta = DummyTrackMeta;

    const TYPE_ID: &'static str = "dummy.source";
    const DISPLAY_NAME: &'static str = "Dummy Source";
    const CONFIG_SCHEMA_JSON: &'static str = "{}";

    fn default_config() -> Self::Config {
        DummySourceConfig::default()
    }

    fn list_items(
        _config: &Self::Config,
        _request: &Self::ListRequest,
    ) -> SdkResult<Vec<Self::ListItem>> {
        Ok(vec![DummyListItem {
            id: "dummy-track".to_string(),
            title: "Dummy Track".to_string(),
        }])
    }

    fn open_stream(
        _config: &Self::Config,
        _track: &Self::Track,
    ) -> SdkResult<SourceOpenResult<Self::Stream, Self::TrackMeta>> {
        Ok(
            SourceOpenResult::new(DummySourceStream::with_bytes(b"dummy-source-bytes"))
                .with_track_meta(DummyTrackMeta {
                    title: "Dummy Track".to_string(),
                }),
        )
    }
}

crate::export_output_sinks_interface! {
    sinks: [
        dummy => DummyOutputSink,
    ],
}

crate::export_source_catalogs_interface! {
    sources: [
        dummy_source => DummySourceCatalog,
    ],
}

crate::compose_get_interface! {
    fn __st_get_interface;
    __st_output_sinks_get_interface,
    __st_source_catalogs_get_interface,
}

fn ststr_ref(s: &str) -> StStr {
    StStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

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

#[test]
fn exported_output_sink_interface_works() {
    let iface = __st_get_interface(ststr(ST_INTERFACE_OUTPUT_SINKS_V1));
    assert!(!iface.is_null());
    let registry = iface as *const StOutputSinkRegistryV1;
    assert!(!registry.is_null());
    let count = unsafe { ((*registry).output_sink_count)() };
    assert_eq!(count, 1);
    let vt = unsafe { ((*registry).output_sink_get)(0) };
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
    let mut negotiated = StOutputSinkNegotiatedSpecV1 {
        spec: StAudioSpec {
            sample_rate: 0,
            channels: 0,
            reserved: 0,
        },
        preferred_chunk_frames: 0,
        flags: 0,
        reserved: 0,
    };
    let st = unsafe { ((*vt).negotiate_spec)(cfg, target, spec, &mut negotiated) };
    assert_eq!(st.code, 0);
    assert_eq!(negotiated.spec.sample_rate, spec.sample_rate);
    assert_eq!(negotiated.spec.channels, spec.channels);

    let mut handle: *mut c_void = core::ptr::null_mut();
    let st = unsafe { ((*vt).open)(cfg, target, negotiated.spec, &mut handle) };
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

#[test]
fn exported_source_catalog_interface_works() {
    let iface = __st_get_interface(ststr(ST_INTERFACE_SOURCE_CATALOGS_V1));
    assert!(!iface.is_null());

    let registry = iface as *const StSourceCatalogRegistryV1;
    assert!(!registry.is_null());
    let count = unsafe { ((*registry).source_catalog_count)() };
    assert_eq!(count, 1);
    let vt = unsafe { ((*registry).source_catalog_get)(0) };
    assert!(!vt.is_null());

    let cfg = ststr_ref("{}");
    let req = ststr_ref("{}");
    let mut out_json = StStr::empty();
    let st = unsafe { ((*vt).list_items_json_utf8)(cfg, req, &mut out_json) };
    assert_eq!(st.code, 0);
    let items = unsafe { ststr_to_str(&out_json) }.expect("items utf8");
    assert!(items.contains("dummy-track"));
    plugin_free(out_json.ptr as *mut c_void, out_json.len, 1);

    let track = ststr_ref(r#"{"id":"dummy-track"}"#);
    let mut out_io_vtable: *const StIoVTableV1 = core::ptr::null();
    let mut out_io_handle: *mut c_void = core::ptr::null_mut();
    let mut out_track_meta = StStr::empty();
    let st = unsafe {
        ((*vt).open_stream)(
            cfg,
            track,
            &mut out_io_vtable,
            &mut out_io_handle,
            &mut out_track_meta,
        )
    };
    assert_eq!(st.code, 0);
    assert!(!out_io_vtable.is_null());
    assert!(!out_io_handle.is_null());

    if !out_track_meta.ptr.is_null() && out_track_meta.len > 0 {
        let meta = unsafe { ststr_to_str(&out_track_meta) }.expect("meta utf8");
        assert!(meta.contains("Dummy Track"));
        plugin_free(out_track_meta.ptr as *mut c_void, out_track_meta.len, 1);
    }

    let mut out_size = 0u64;
    let size = unsafe { (*out_io_vtable).size }.expect("size callback");
    let st = size(out_io_handle, &mut out_size as *mut u64);
    assert_eq!(st.code, 0);
    assert_eq!(out_size, "dummy-source-bytes".len() as u64);

    let mut buf = [0u8; 5];
    let mut out_read = 0usize;
    let st = unsafe {
        ((*out_io_vtable).read)(
            out_io_handle,
            buf.as_mut_ptr(),
            buf.len(),
            &mut out_read as *mut usize,
        )
    };
    assert_eq!(st.code, 0);
    assert_eq!(out_read, 5);
    assert_eq!(&buf, b"dummy");

    let seek = unsafe { (*out_io_vtable).seek }.expect("seek callback");
    let mut out_pos = 0u64;
    let st = seek(
        out_io_handle,
        -5,
        StSeekWhence::End,
        &mut out_pos as *mut u64,
    );
    assert_eq!(st.code, 0);
    assert_eq!(out_pos, ("dummy-source-bytes".len() - 5) as u64);

    let tell = unsafe { (*out_io_vtable).tell }.expect("tell callback");
    let mut tell_pos = 0u64;
    let st = tell(out_io_handle, &mut tell_pos as *mut u64);
    assert_eq!(st.code, 0);
    assert_eq!(tell_pos, out_pos);

    unsafe { ((*vt).close_stream)(out_io_handle) };
}
