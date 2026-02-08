pub use stellatune_plugin_api::*;

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use serde_json::{Map, Value};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{SystemTime, UNIX_EPOCH};
use stellatune_core::{
    ControlScope, HostControlFinishedPayload, HostControlResultPayload, HostEventTopic,
    HostLibraryEventEnvelope, HostPlayerEventEnvelope, HostPlayerTickPayload,
    LibraryControlCommand, PlayerControlCommand,
};

static HOST_VTABLE_V1: AtomicPtr<StHostVTableV1> = AtomicPtr::new(core::ptr::null_mut());
static REQUEST_ID_SEQ: AtomicU64 = AtomicU64::new(1);

#[doc(hidden)]
pub unsafe fn __set_host_vtable_v1(host: *const StHostVTableV1) {
    HOST_VTABLE_V1.store(host as *mut StHostVTableV1, Ordering::Release);
}

/// Log a message to the host, if the host provided a logger.
///
/// This is purely best-effort: if no host logger is present, this is a no-op.
pub fn host_log(level: StLogLevel, msg: &str) {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return;
    }

    // Safety: the host owns the vtable and defines its lifetime.
    let cb = unsafe { (*host).log_utf8 };
    let Some(cb) = cb else {
        return;
    };

    let bytes = msg.as_bytes();
    let st = StStr {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
    };
    let user_data = unsafe { (*host).user_data };
    cb(user_data, level, st);
}

#[macro_export]
macro_rules! host_log {
    ($lvl:expr, $($arg:tt)*) => {{
        $crate::host_log($lvl, &format!($($arg)*));
    }};
}

/// Returns runtime root directory assigned by host for this plugin.
pub fn plugin_runtime_root() -> Option<String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return None;
    }
    let cb = unsafe { (*host).get_runtime_root_utf8 }?;
    let user_data = unsafe { (*host).user_data };
    let root = cb(user_data);
    if root.ptr.is_null() || root.len == 0 {
        return None;
    }
    unsafe { ststr_to_str(&root).ok().map(ToOwned::to_owned) }
}

/// Returns runtime root directory assigned by host for this plugin as `PathBuf`.
pub fn plugin_runtime_root_path() -> Option<PathBuf> {
    plugin_runtime_root().map(PathBuf::from)
}

fn host_take_owned_string(host: *const StHostVTableV1, s: StStr) -> String {
    if s.ptr.is_null() || s.len == 0 {
        return String::new();
    }

    let text = unsafe { ststr_to_str(&s) }
        .map(ToOwned::to_owned)
        .unwrap_or_else(|_| String::new());

    let free_cb = unsafe { (*host).free_host_str_utf8 };
    if let Some(free_cb) = free_cb {
        let user_data = unsafe { (*host).user_data };
        free_cb(user_data, s);
    }

    text
}

fn host_status_to_result(
    host: *const StHostVTableV1,
    what: &str,
    status: StStatus,
) -> Result<(), String> {
    if status.code == 0 {
        return Ok(());
    }
    let msg = host_take_owned_string(host, status.message);
    if msg.is_empty() {
        Err(format!("{what} failed (code={})", status.code))
    } else {
        Err(format!("{what} failed (code={}): {msg}", status.code))
    }
}

/// Emit runtime event JSON to host (plugin -> host).
pub fn host_emit_event_json(event_json: &str) -> Result<(), String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return Err("host vtable unavailable".to_string());
    }
    let cb = unsafe { (*host).emit_event_json_utf8 }
        .ok_or_else(|| "host callback `emit_event_json_utf8` unavailable".to_string())?;
    let user_data = unsafe { (*host).user_data };
    let in_json = StStr {
        ptr: event_json.as_ptr(),
        len: event_json.len(),
    };
    let status = cb(user_data, in_json);
    host_status_to_result(host, "emit_event_json_utf8", status)
}

/// Poll one host event JSON (host/flutter -> plugin).
pub fn host_poll_event_json() -> Result<Option<String>, String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return Err("host vtable unavailable".to_string());
    }
    let cb = unsafe { (*host).poll_host_event_json_utf8 }
        .ok_or_else(|| "host callback `poll_host_event_json_utf8` unavailable".to_string())?;
    let user_data = unsafe { (*host).user_data };
    let mut out = StStr::empty();
    let status = cb(user_data, &mut out as *mut StStr);
    host_status_to_result(host, "poll_host_event_json_utf8", status)?;
    if out.ptr.is_null() || out.len == 0 {
        return Ok(None);
    }
    Ok(Some(host_take_owned_string(host, out)))
}

/// Send control request JSON to host and receive immediate response JSON.
pub fn host_send_control_json(request_json: &str) -> Result<String, String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return Err("host vtable unavailable".to_string());
    }
    let cb = unsafe { (*host).send_control_json_utf8 }
        .ok_or_else(|| "host callback `send_control_json_utf8` unavailable".to_string())?;
    let user_data = unsafe { (*host).user_data };
    let in_json = StStr {
        ptr: request_json.as_ptr(),
        len: request_json.len(),
    };
    let mut out = StStr::empty();
    let status = cb(user_data, in_json, &mut out as *mut StStr);
    host_status_to_result(host, "send_control_json_utf8", status)?;
    Ok(host_take_owned_string(host, out))
}

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

/// Resolves a path relative to plugin runtime root.
pub fn resolve_runtime_path(relative: impl AsRef<Path>) -> Option<PathBuf> {
    let root = plugin_runtime_root_path()?;
    let rel = relative.as_ref();
    if rel.as_os_str().is_empty() {
        return Some(root);
    }
    if rel.is_absolute() {
        return Some(rel.to_path_buf());
    }
    Some(root.join(rel))
}

/// Build a command to launch a sidecar program under plugin runtime root.
///
/// The current working directory is set to runtime root.
pub fn sidecar_command(relative_program: impl AsRef<Path>) -> io::Result<Command> {
    let root = plugin_runtime_root_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "plugin runtime root is unavailable",
        )
    })?;
    let program = root.join(relative_program.as_ref());
    let mut cmd = Command::new(program);
    cmd.current_dir(root);
    Ok(cmd)
}

/// Spawn a sidecar program under plugin runtime root.
pub fn spawn_sidecar<I, S>(relative_program: impl AsRef<Path>, args: I) -> io::Result<Child>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = sidecar_command(relative_program)?;
    cmd.args(args);
    cmd.spawn()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PluginMetadataVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub api_version: u32,
    pub version: PluginMetadataVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<serde_json::Value>,
}

impl PluginMetadata {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

pub fn build_plugin_metadata(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
) -> PluginMetadata {
    PluginMetadata {
        id: id.into(),
        name: name.into(),
        api_version: STELLATUNE_PLUGIN_API_VERSION_V1,
        version: PluginMetadataVersion {
            major,
            minor,
            patch,
        },
        info: None,
    }
}

pub fn build_plugin_metadata_with_info(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
    info: Option<serde_json::Value>,
) -> PluginMetadata {
    let mut meta = build_plugin_metadata(id, name, major, minor, patch);
    meta.info = info;
    meta
}

pub fn build_plugin_metadata_json(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
) -> String {
    let meta = build_plugin_metadata(id, name, major, minor, patch);
    match meta.to_json() {
        Ok(s) => s,
        Err(_) => {
            let id = meta.id.replace('\\', "\\\\").replace('"', "\\\"");
            let name = meta.name.replace('\\', "\\\\").replace('"', "\\\"");
            format!(
                r#"{{"id":"{id}","name":"{name}","api_version":{},"version":{{"major":{},"minor":{},"patch":{}}}}}"#,
                meta.api_version, meta.version.major, meta.version.minor, meta.version.patch
            )
        }
    }
}

pub fn build_plugin_metadata_json_with_info_json(
    id: impl Into<String>,
    name: impl Into<String>,
    major: u16,
    minor: u16,
    patch: u16,
    info_json: Option<&str>,
) -> String {
    let info = info_json.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(v) => Some(v),
            Err(_) => Some(serde_json::Value::String(trimmed.to_string())),
        }
    });
    let meta = build_plugin_metadata_with_info(id, name, major, minor, patch, info);
    match meta.to_json() {
        Ok(s) => s,
        Err(_) => build_plugin_metadata_json(
            meta.id,
            meta.name,
            meta.version.major,
            meta.version.minor,
            meta.version.patch,
        ),
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __st_opt_get_interface {
    () => {
        None
    };
    ($f:path) => {
        Some($f)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __st_opt_info_json {
    () => {
        None::<&str>
    };
    ($v:expr) => {
        Some($v)
    };
}

pub trait Dsp: Send + 'static {
    fn set_config_json(&mut self, _json: &str) -> Result<(), String> {
        Ok(())
    }

    fn process_interleaved_f32_in_place(&mut self, samples: &mut [f32], frames: u32);
}

pub trait DspDescriptor: Dsp {
    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str;

    /// Bitmask of supported channel layouts (ST_LAYOUT_* flags).
    /// Default: ST_LAYOUT_STEREO (stereo only).
    const SUPPORTED_LAYOUTS: u32 = ST_LAYOUT_STEREO;

    /// Output channel count if this DSP changes channel count.
    /// Return 0 to preserve input channel count (passthrough).
    const OUTPUT_CHANNELS: u16 = 0;

    fn create(spec: StAudioSpec, config_json: &str) -> Result<Self, String>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderInfo {
    pub spec: StAudioSpec,
    pub duration_ms: Option<u64>,
    pub seekable: bool,
}

impl DecoderInfo {
    pub fn to_ffi(self) -> StDecoderInfoV1 {
        let mut flags = 0u32;
        if self.seekable {
            flags |= ST_DECODER_INFO_FLAG_SEEKABLE;
        }
        let mut duration_ms = 0u64;
        if let Some(d) = self.duration_ms {
            flags |= ST_DECODER_INFO_FLAG_HAS_DURATION;
            duration_ms = d;
        }
        StDecoderInfoV1 {
            spec: self.spec,
            duration_ms,
            flags,
            reserved: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct HostIo {
    vtable: *const StIoVTableV1,
    handle: *mut c_void,
}

unsafe impl Send for HostIo {}
// Raw pointers make this not auto-Sync. StellaTune v1 treats the IO vtable as immutable, and the
// host must ensure any IO handle is thread-safe if it is accessed from multiple threads.
unsafe impl Sync for HostIo {}

impl HostIo {
    pub unsafe fn from_raw(vtable: *const StIoVTableV1, handle: *mut c_void) -> Self {
        Self { vtable, handle }
    }

    pub fn is_seekable(self) -> bool {
        if self.vtable.is_null() {
            return false;
        }
        unsafe { (*self.vtable).seek.is_some() }
    }

    pub fn size(self) -> io::Result<u64> {
        if self.vtable.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "null io_vtable",
            ));
        }
        let Some(size) = (unsafe { (*self.vtable).size }) else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "size unsupported",
            ));
        };
        let mut out = 0u64;
        let st = (size)(self.handle, &mut out);
        if st.code != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("size failed (code={})", st.code),
            ));
        }
        Ok(out)
    }
}

impl Read for HostIo {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.vtable.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "null io_vtable",
            ));
        }
        let mut out_read: usize = 0;
        let st = unsafe {
            ((*self.vtable).read)(self.handle, buf.as_mut_ptr(), buf.len(), &mut out_read)
        };
        if st.code != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("read failed (code={})", st.code),
            ));
        }
        Ok(out_read)
    }
}

impl Seek for HostIo {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if self.vtable.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "null io_vtable",
            ));
        }
        let Some(seek) = (unsafe { (*self.vtable).seek }) else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "seek unsupported",
            ));
        };
        let (offset, whence) = match pos {
            SeekFrom::Start(n) => (n as i64, StSeekWhence::Start),
            SeekFrom::Current(n) => (n, StSeekWhence::Current),
            SeekFrom::End(n) => (n, StSeekWhence::End),
        };
        let mut out_pos = 0u64;
        let st = (seek)(self.handle, offset, whence, &mut out_pos);
        if st.code != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("seek failed (code={})", st.code),
            ));
        }
        Ok(out_pos)
    }
}

pub struct DecoderOpenArgs<'a> {
    pub path: &'a str,
    pub ext: &'a str,
    pub io: HostIo,
}

pub trait Decoder: Send + 'static {
    fn info(&self) -> DecoderInfo;

    fn seek_ms(&mut self, _position_ms: u64) -> Result<(), String> {
        Err("seek not supported".to_string())
    }

    fn metadata_json(&self) -> Option<String> {
        None
    }

    /// Fill `out_interleaved` with up to `frames` frames.
    /// Returns `(frames_written, eof)`.
    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> Result<(u32, bool), String>;
}

pub trait DecoderDescriptor: Decoder {
    const TYPE_ID: &'static str;
    const SUPPORTS_SEEK: bool = true;

    fn probe(_path_ext: &str, _header: &[u8]) -> u8 {
        0
    }

    fn open(args: DecoderOpenArgs<'_>) -> Result<Self, String>
    where
        Self: Sized;
}

#[inline]
pub const fn ststr(s: &'static str) -> StStr {
    StStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

#[inline]
pub fn status_ok() -> StStatus {
    StStatus::ok()
}

#[inline]
pub fn status_err(code: i32) -> StStatus {
    StStatus {
        code,
        message: StStr::empty(),
    }
}

pub extern "C" fn plugin_free(ptr: *mut c_void, len: usize, align: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let align = align.max(1);
    // Safety: allocated by `alloc_utf8_bytes` with the same layout.
    unsafe {
        let layout = std::alloc::Layout::from_size_align_unchecked(len, align);
        std::alloc::dealloc(ptr as *mut u8, layout);
    }
}

pub fn alloc_utf8_bytes(s: &str) -> StStr {
    if s.is_empty() {
        return StStr::empty();
    }
    let bytes = s.as_bytes();
    let len = bytes.len();
    let layout = std::alloc::Layout::from_size_align(len, 1).expect("layout");
    // Safety: layout is valid, and we copy exactly `len` bytes.
    unsafe {
        let ptr = std::alloc::alloc(layout);
        if ptr.is_null() {
            return StStr::empty();
        }
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len);
        StStr { ptr, len }
    }
}

pub fn status_err_msg(code: i32, msg: &str) -> StStatus {
    StStatus {
        code,
        message: alloc_utf8_bytes(msg),
    }
}

pub unsafe fn ststr_to_str<'a>(s: &'a StStr) -> Result<&'a str, String> {
    if s.ptr.is_null() || s.len == 0 {
        return Ok("");
    }
    let bytes = unsafe { core::slice::from_raw_parts(s.ptr, s.len) };
    core::str::from_utf8(bytes).map_err(|_| "invalid utf-8".to_string())
}

#[doc(hidden)]
pub struct DspBox<T: Dsp> {
    pub inner: T,
    pub channels: u16,
}

#[doc(hidden)]
#[allow(dead_code)]
pub struct DecoderBox<T: Decoder> {
    pub inner: T,
    pub channels: u16,
}

/// Export a StellaTune plugin (v1) with decoder + DSP types.
///
/// Syntax:
/// ```ignore
/// export_plugin! {
///   id: "com.example.gain",
///   name: "Gain",
///   version: (0, 1, 0),
///   decoders: [
///     tone => ToneDecoder,
///   ],
///   dsps: [
///     gain => GainDsp,
///   ]
///   // Optional free-form JSON for UI display.
///   // info_json: r#"{"author":"StellaTune Team","homepage":"https://example.com"}"#,
///   // Optional advanced interfaces (source/lyrics/output) can be exposed by
///   // implementing a custom `get_interface` callback.
///   // get_interface: my_get_interface,
/// }
/// ```
#[macro_export]
macro_rules! export_plugin {
    (
        id: $plugin_id:literal,
        name: $plugin_name:literal,
        version: ($vmaj:literal, $vmin:literal, $vpatch:literal),
        decoders: [
            $($dec_mod:ident => $dec_ty:ty),* $(,)?
        ],
        dsps: [
            $($dsp_mod:ident => $dsp_ty:ty),* $(,)?
        ]
        $(, info_json: $info_json:expr)?
        $(, get_interface: $get_interface:path)?
        $(,)?
    ) => {
        const __ST_PLUGIN_ID: &str = $plugin_id;
        const __ST_PLUGIN_NAME: &str = $plugin_name;

        extern "C" fn __st_plugin_id_utf8() -> $crate::StStr {
            $crate::ststr(__ST_PLUGIN_ID)
        }

        extern "C" fn __st_plugin_name_utf8() -> $crate::StStr {
            $crate::ststr(__ST_PLUGIN_NAME)
        }

        fn __st_plugin_metadata_json() -> &'static str {
            static META: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            META.get_or_init(|| {
                $crate::build_plugin_metadata_json_with_info_json(
                    __ST_PLUGIN_ID,
                    __ST_PLUGIN_NAME,
                    $vmaj,
                    $vmin,
                    $vpatch,
                    $crate::__st_opt_info_json!($($info_json)?),
                )
            })
        }

        extern "C" fn __st_plugin_metadata_json_utf8() -> $crate::StStr {
            let s = __st_plugin_metadata_json();
            $crate::StStr {
                ptr: s.as_ptr(),
                len: s.len(),
            }
        }

        $(
            mod $dec_mod {
                use super::*;

                extern "C" fn type_id_utf8() -> $crate::StStr {
                    $crate::ststr(<$dec_ty as $crate::DecoderDescriptor>::TYPE_ID)
                }

                extern "C" fn probe(path_ext_utf8: $crate::StStr, header: $crate::StSlice<u8>) -> u8 {
                    let ext = unsafe { $crate::ststr_to_str(&path_ext_utf8) }.unwrap_or("");
                    let bytes = if header.ptr.is_null() || header.len == 0 {
                        &[][..]
                    } else {
                        unsafe { core::slice::from_raw_parts(header.ptr, header.len) }
                    };
                    <$dec_ty as $crate::DecoderDescriptor>::probe(ext, bytes)
                }

                extern "C" fn open(
                    args: $crate::StDecoderOpenArgsV1,
                    out: *mut *mut core::ffi::c_void,
                ) -> $crate::StStatus {
                    if out.is_null() || args.io_vtable.is_null() || args.io_handle.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "invalid open args",
                        );
                    }

                    let path = match unsafe { $crate::ststr_to_str(&args.path_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let ext = match unsafe { $crate::ststr_to_str(&args.ext_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let io = unsafe { $crate::HostIo::from_raw(args.io_vtable, args.io_handle) };

                    match <$dec_ty as $crate::DecoderDescriptor>::open($crate::DecoderOpenArgs {
                        path,
                        ext,
                        io,
                    }) {
                        Ok(dec) => {
                            let info = <$dec_ty as $crate::Decoder>::info(&dec);
                            let boxed = Box::new($crate::DecoderBox {
                                inner: dec,
                                channels: info.spec.channels.max(1),
                            });
                            unsafe { *out = Box::into_raw(boxed) as *mut core::ffi::c_void; }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_DECODE, &e),
                    }
                }

                extern "C" fn get_info(
                    handle: *mut core::ffi::c_void,
                    out_info: *mut $crate::StDecoderInfoV1,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_info.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle",
                        );
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    let info = <$dec_ty as $crate::Decoder>::info(&boxed.inner).to_ffi();
                    unsafe { *out_info = info; }
                    $crate::status_ok()
                }

                extern "C" fn get_metadata_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle",
                        );
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::Decoder>::metadata_json(&boxed.inner) {
                        None => {
                            unsafe { *out_json = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Some(s) => {
                            unsafe { *out_json = $crate::alloc_utf8_bytes(&s); }
                            $crate::status_ok()
                        }
                    }
                }

                extern "C" fn read_interleaved_f32(
                    handle: *mut core::ffi::c_void,
                    frames: u32,
                    out_interleaved: *mut f32,
                    out_frames_read: *mut u32,
                    out_eof: *mut bool,
                ) -> $crate::StStatus {
                    if handle.is_null()
                        || out_interleaved.is_null()
                        || out_frames_read.is_null()
                        || out_eof.is_null()
                    {
                        return $crate::status_err(-1);
                    }

                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    let len = (frames as usize).saturating_mul(boxed.channels as usize);
                    let out = unsafe { core::slice::from_raw_parts_mut(out_interleaved, len) };

                    match <$dec_ty as $crate::Decoder>::read_interleaved_f32(
                        &mut boxed.inner,
                        frames,
                        out,
                    ) {
                        Ok((n, eof)) => {
                            unsafe {
                                *out_frames_read = n;
                                *out_eof = eof;
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_DECODE, &e),
                    }
                }

                extern "C" fn seek_ms(handle: *mut core::ffi::c_void, position_ms: u64) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle",
                        );
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::Decoder>::seek_ms(&mut boxed.inner, position_ms) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, &e),
                    }
                }

                extern "C" fn close(handle: *mut core::ffi::c_void) {
                    if handle.is_null() { return; }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::DecoderBox<$dec_ty>)) };
                }

                pub static VTABLE: $crate::StDecoderVTableV1 = $crate::StDecoderVTableV1 {
                    type_id_utf8,
                    probe,
                    open,
                    get_info,
                    get_metadata_json_utf8: Some(get_metadata_json_utf8),
                    read_interleaved_f32,
                    seek_ms: if <$dec_ty as $crate::DecoderDescriptor>::SUPPORTS_SEEK {
                        Some(seek_ms)
                    } else {
                        None
                    },
                    close,
                };
            }
        )*

        const __ST_DEC_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dec_ty>(); 1 })*;
        extern "C" fn __st_decoder_count() -> usize { __ST_DEC_COUNT }
        extern "C" fn __st_decoder_get(index: usize) -> *const $crate::StDecoderVTableV1 {
            let mut i = 0usize;
            $(
                if index == i {
                    return &$dec_mod::VTABLE;
                }
                i += 1;
            )*
            core::ptr::null()
        }

        $(
            mod $dsp_mod {
                use super::*;

                extern "C" fn type_id_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::TYPE_ID)
                }
                extern "C" fn display_name_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::DISPLAY_NAME)
                }
                extern "C" fn config_schema_json_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::CONFIG_SCHEMA_JSON)
                }
                extern "C" fn default_config_json_utf8() -> $crate::StStr {
                    $crate::ststr(<$dsp_ty as $crate::DspDescriptor>::DEFAULT_CONFIG_JSON)
                }

                extern "C" fn create(
                    sample_rate: u32,
                    channels: u16,
                    config_json_utf8: $crate::StStr,
                    out: *mut *mut core::ffi::c_void,
                ) -> $crate::StStatus {
                    if out.is_null() {
                        return $crate::status_err(-1);
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(_) => return $crate::status_err(-2),
                    };
                    let spec = $crate::StAudioSpec {
                        sample_rate,
                        channels,
                        reserved: 0,
                    };
                    let channels = channels.max(1);

                    match <$dsp_ty as $crate::DspDescriptor>::create(spec, json) {
                        Ok(dsp) => {
                            let boxed = Box::new($crate::DspBox {
                                inner: dsp,
                                channels,
                            });
                            unsafe { *out = Box::into_raw(boxed) as *mut core::ffi::c_void; }
                            $crate::status_ok()
                        }
                        Err(_) => $crate::status_err(-3),
                    }
                }

                extern "C" fn set_config_json_utf8(
                    handle: *mut core::ffi::c_void,
                    config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err(-1);
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(_) => return $crate::status_err(-2),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::DspBox<$dsp_ty>) };
                    match <$dsp_ty as $crate::Dsp>::set_config_json(&mut boxed.inner, json) {
                        Ok(()) => $crate::status_ok(),
                        Err(_) => $crate::status_err(-3),
                    }
                }

                extern "C" fn process_interleaved_f32_in_place(
                    handle: *mut core::ffi::c_void,
                    samples: *mut f32,
                    frames: u32,
                ) {
                    if handle.is_null() || samples.is_null() {
                        return;
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::DspBox<$dsp_ty>) };
                    let len = (frames as usize).saturating_mul(boxed.channels as usize);
                    let buf = unsafe { core::slice::from_raw_parts_mut(samples, len) };
                    <$dsp_ty as $crate::Dsp>::process_interleaved_f32_in_place(
                        &mut boxed.inner,
                        buf,
                        frames,
                    );
                }

                extern "C" fn drop_handle(handle: *mut core::ffi::c_void) {
                    if handle.is_null() { return; }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::DspBox<$dsp_ty>)) };
                }

                extern "C" fn supported_layouts() -> u32 {
                    <$dsp_ty as $crate::DspDescriptor>::SUPPORTED_LAYOUTS
                }

                extern "C" fn output_channels() -> u16 {
                    <$dsp_ty as $crate::DspDescriptor>::OUTPUT_CHANNELS
                }

                pub static VTABLE: $crate::StDspVTableV1 = $crate::StDspVTableV1 {
                    type_id_utf8,
                    display_name_utf8,
                    config_schema_json_utf8,
                    default_config_json_utf8,
                    create,
                    set_config_json_utf8,
                    process_interleaved_f32_in_place,
                    drop: drop_handle,
                    supported_layouts,
                    output_channels,
                };
            }
        )*

        const __ST_DSP_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dsp_ty>(); 1 })*;
        extern "C" fn __st_dsp_count() -> usize { __ST_DSP_COUNT }

        extern "C" fn __st_dsp_get(index: usize) -> *const $crate::StDspVTableV1 {
            let mut i = 0usize;
            $(
                if index == i {
                    return &$dsp_mod::VTABLE;
                }
                i += 1;
            )*
            core::ptr::null()
        }

        static __ST_PLUGIN_VTABLE: $crate::StPluginVTableV1 = $crate::StPluginVTableV1 {
            api_version: $crate::STELLATUNE_PLUGIN_API_VERSION_V1,
            plugin_version: $crate::StVersion { major: $vmaj, minor: $vmin, patch: $vpatch, reserved: 0 },
            plugin_free: Some($crate::plugin_free),
            id_utf8: __st_plugin_id_utf8,
            name_utf8: __st_plugin_name_utf8,
            metadata_json_utf8: __st_plugin_metadata_json_utf8,
            decoder_count: __st_decoder_count,
            decoder_get: __st_decoder_get,
            dsp_count: __st_dsp_count,
            dsp_get: __st_dsp_get,
            get_interface: $crate::__st_opt_get_interface!($($get_interface)?),
        };

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn stellatune_plugin_entry_v1(
            host: *const $crate::StHostVTableV1,
        ) -> *const $crate::StPluginVTableV1 {
            unsafe { $crate::__set_host_vtable_v1(host) };
            &__ST_PLUGIN_VTABLE
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
