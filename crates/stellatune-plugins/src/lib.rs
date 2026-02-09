mod manifest;
pub mod runtime;
mod util;
pub mod v2;

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use libloading::{Library, Symbol};
use serde::{Serialize, de::DeserializeOwned};
use stellatune_core::{PluginRuntimeEvent, PluginRuntimeKind};
use stellatune_plugin_api::{
    ST_ERR_INTERNAL, ST_ERR_INVALID_ARG, ST_ERR_IO, ST_INTERFACE_LYRICS_PROVIDER_V1,
    ST_INTERFACE_OUTPUT_SINKS_V1, ST_INTERFACE_SOURCE_CATALOGS_V1, ST_OUTPUT_NEGOTIATE_CHANGED_CH,
    ST_OUTPUT_NEGOTIATE_CHANGED_SR, ST_OUTPUT_NEGOTIATE_EXACT, STELLATUNE_PLUGIN_API_VERSION_V1,
    STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1, StAudioSpec, StDecoderInfoV1, StDecoderOpenArgsV1,
    StDecoderVTableV1, StHostVTableV1, StIoVTableV1, StLogLevel, StLyricsProviderVTableV1,
    StOutputSinkNegotiatedSpecV1, StOutputSinkRegistryV1, StOutputSinkVTableV1, StPluginEntryV1,
    StPluginVTableV1, StSeekWhence, StSourceCatalogRegistryV1, StSourceCatalogVTableV1, StStatus,
    StStr,
};
use stellatune_plugin_protocol::PluginMetadata;
use tracing::{debug, info, warn};

use crate::runtime::{
    CapabilityKind as RuntimeCapabilityKind, GenerationGuard, GenerationId, InstanceId,
    InstanceRegistry, LifecycleStore,
};

pub use manifest::{
    DiscoveredPlugin, INSTALL_RECEIPT_FILE_NAME, PluginInstallReceipt, PluginInstallState,
    PluginManifest, UNINSTALL_PENDING_MARKER_FILE_NAME, UninstallPendingMarker,
    discover_pending_uninstalls, discover_plugins, pending_marker_path_for_plugin_root,
    read_receipt, read_uninstall_pending_marker, receipt_path_for_plugin_root, write_receipt,
    write_uninstall_pending_marker,
};

extern "C" fn default_host_log(_: *mut core::ffi::c_void, level: StLogLevel, msg: StStr) {
    let text = unsafe { util::ststr_to_string_lossy(msg) };
    match level {
        StLogLevel::Error => tracing::error!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Warn => tracing::warn!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Info => tracing::info!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Debug => tracing::debug!(target: "stellatune_plugins::plugin", "{text}"),
        StLogLevel::Trace => tracing::trace!(target: "stellatune_plugins::plugin", "{text}"),
    }
}

pub fn default_host_vtable() -> StHostVTableV1 {
    StHostVTableV1 {
        api_version: STELLATUNE_PLUGIN_API_VERSION_V1,
        user_data: core::ptr::null_mut(),
        log_utf8: Some(default_host_log),
        get_runtime_root_utf8: None,
        emit_event_json_utf8: None,
        poll_host_event_json_utf8: None,
        send_control_json_utf8: None,
        free_host_str_utf8: None,
    }
}

#[derive(Debug, Clone)]
struct PluginHostCtx {
    plugin_id: String,
    runtime_root_utf8: Box<[u8]>,
    event_bus: PluginEventBus,
}

extern "C" fn plugin_host_runtime_root(user_data: *mut core::ffi::c_void) -> StStr {
    if user_data.is_null() {
        return StStr::empty();
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    if ctx.runtime_root_utf8.is_empty() {
        return StStr::empty();
    }
    StStr {
        ptr: ctx.runtime_root_utf8.as_ptr(),
        len: ctx.runtime_root_utf8.len(),
    }
}

extern "C" fn plugin_host_emit_event_json(
    user_data: *mut core::ffi::c_void,
    event_json_utf8: StStr,
) -> StStatus {
    if user_data.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let payload = unsafe { util::ststr_to_string_lossy(event_json_utf8) };
    ctx.event_bus.push_plugin_event(PluginRuntimeEvent {
        plugin_id: ctx.plugin_id.clone(),
        kind: PluginRuntimeKind::Notify,
        payload_json: payload,
    });
    StStatus::ok()
}

extern "C" fn plugin_host_poll_event_json(
    user_data: *mut core::ffi::c_void,
    out_event_json_utf8: *mut StStr,
) -> StStatus {
    if user_data.is_null() || out_event_json_utf8.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let out = match ctx.event_bus.poll_host_event(&ctx.plugin_id) {
        Some(event_json) => alloc_host_owned_ststr(&event_json),
        None => StStr::empty(),
    };
    unsafe {
        *out_event_json_utf8 = out;
    }
    StStatus::ok()
}

extern "C" fn plugin_host_send_control_json(
    user_data: *mut core::ffi::c_void,
    request_json_utf8: StStr,
    out_response_json_utf8: *mut StStr,
) -> StStatus {
    if user_data.is_null() || out_response_json_utf8.is_null() {
        return StStatus {
            code: ST_ERR_INVALID_ARG,
            message: StStr::empty(),
        };
    }
    let ctx = unsafe { &*(user_data as *const PluginHostCtx) };
    let payload = unsafe { util::ststr_to_string_lossy(request_json_utf8) };
    ctx.event_bus.push_plugin_event(PluginRuntimeEvent {
        plugin_id: ctx.plugin_id.clone(),
        kind: PluginRuntimeKind::Control,
        payload_json: payload,
    });
    let ack_json = r#"{"ok":true}"#;
    unsafe {
        *out_response_json_utf8 = alloc_host_owned_ststr(ack_json);
    }
    StStatus::ok()
}

extern "C" fn plugin_host_free_str(_user_data: *mut core::ffi::c_void, s: StStr) {
    free_host_owned_ststr(s);
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledPluginInfo {
    pub id: String,
    pub name: String,
    pub root_dir: PathBuf,
    pub library_path: PathBuf,
    pub info_json: Option<String>,
    pub install_state: PluginInstallState,
    pub uninstall_retry_count: u32,
    pub uninstall_last_error: Option<String>,
}

#[derive(Debug, Default)]
struct PluginEventBusState {
    host_to_plugin: HashMap<String, VecDeque<String>>,
    plugin_to_host: VecDeque<PluginRuntimeEvent>,
}

#[derive(Debug, Clone)]
struct PluginEventBus {
    inner: Arc<Mutex<PluginEventBusState>>,
    per_plugin_queue_cap: usize,
    outbound_queue_cap: usize,
}

impl PluginEventBus {
    fn new(per_plugin_queue_cap: usize, outbound_queue_cap: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PluginEventBusState::default())),
            per_plugin_queue_cap,
            outbound_queue_cap,
        }
    }

    fn register_plugin(&self, plugin_id: &str) {
        if let Ok(mut state) = self.inner.lock() {
            state
                .host_to_plugin
                .entry(plugin_id.to_string())
                .or_insert_with(VecDeque::new);
        }
    }

    fn registered_plugin_ids(&self) -> Vec<String> {
        let Ok(state) = self.inner.lock() else {
            return Vec::new();
        };
        state.host_to_plugin.keys().cloned().collect()
    }

    fn push_host_event(&self, plugin_id: &str, event_json: String) {
        if let Ok(mut state) = self.inner.lock() {
            let queue = state
                .host_to_plugin
                .entry(plugin_id.to_string())
                .or_insert_with(VecDeque::new);
            if queue.len() >= self.per_plugin_queue_cap {
                queue.pop_front();
            }
            queue.push_back(event_json);
        }
    }

    fn poll_host_event(&self, plugin_id: &str) -> Option<String> {
        let mut state = self.inner.lock().ok()?;
        state
            .host_to_plugin
            .get_mut(plugin_id)
            .and_then(|queue| queue.pop_front())
    }

    fn push_plugin_event(&self, event: PluginRuntimeEvent) {
        if let Ok(mut state) = self.inner.lock() {
            if state.plugin_to_host.len() >= self.outbound_queue_cap {
                state.plugin_to_host.pop_front();
            }
            state.plugin_to_host.push_back(event);
        }
    }

    fn drain_plugin_events(&self, max: usize) -> Vec<PluginRuntimeEvent> {
        if max == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Ok(mut state) = self.inner.lock() {
            for _ in 0..max {
                let Some(item) = state.plugin_to_host.pop_front() else {
                    break;
                };
                out.push(item);
            }
        }
        out
    }
}

fn shared_plugin_event_bus() -> PluginEventBus {
    static SHARED: OnceLock<PluginEventBus> = OnceLock::new();
    SHARED
        .get_or_init(|| PluginEventBus::new(HOST_TO_PLUGIN_QUEUE_CAP, PLUGIN_TO_HOST_QUEUE_CAP))
        .clone()
}

pub fn drain_shared_runtime_events(max: usize) -> Vec<PluginRuntimeEvent> {
    shared_plugin_event_bus().drain_plugin_events(max)
}

pub fn push_shared_host_event_json(plugin_id: &str, event_json: &str) {
    shared_plugin_event_bus().push_host_event(plugin_id, event_json.to_string());
}

pub fn broadcast_shared_host_event_json(event_json: &str) {
    let bus = shared_plugin_event_bus();
    for plugin_id in bus.registered_plugin_ids() {
        bus.push_host_event(&plugin_id, event_json.to_string());
    }
}

fn alloc_host_owned_ststr(text: &str) -> StStr {
    if text.is_empty() {
        return StStr::empty();
    }
    let boxed = text.as_bytes().to_vec().into_boxed_slice();
    let out = StStr {
        ptr: boxed.as_ptr(),
        len: boxed.len(),
    };
    std::mem::forget(boxed);
    out
}

fn free_host_owned_ststr(s: StStr) {
    if s.ptr.is_null() || s.len == 0 {
        return;
    }
    unsafe {
        let raw = std::ptr::slice_from_raw_parts_mut(s.ptr as *mut u8, s.len);
        drop(Box::from_raw(raw));
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn to_json_string<T: Serialize>(value: &T, what: &str) -> Result<String> {
    serde_json::to_string(value).with_context(|| format!("failed to serialize {what} to json"))
}

fn from_json_str<T: DeserializeOwned>(raw: &str, what: &str) -> Result<T> {
    serde_json::from_str(raw).with_context(|| format!("failed to deserialize {what} from json"))
}

fn metadata_info_json(metadata: Option<&PluginMetadata>) -> Option<String> {
    let meta = metadata?;
    meta.info
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok())
}

pub struct PluginLibrary {
    _lib: Library,
    vtable: *const StPluginVTableV1,
}

fn status_err_to_anyhow(
    what: &str,
    status: StStatus,
    plugin_free: Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)>,
) -> anyhow::Error {
    let msg = unsafe { util::ststr_to_string_lossy(status.message) };
    if status.code != 0
        && status.message.len != 0
        && let Some(free) = plugin_free
    {
        (free)(
            status.message.ptr as *mut core::ffi::c_void,
            status.message.len,
            1,
        );
    }
    if msg.is_empty() {
        anyhow!("{what} failed (code={})", status.code)
    } else {
        anyhow!("{what} failed (code={}): {msg}", status.code)
    }
}

fn status_to_result(
    what: &str,
    status: StStatus,
    plugin_free: Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)>,
) -> Result<()> {
    if status.code == 0 {
        return Ok(());
    }
    Err(status_err_to_anyhow(what, status, plugin_free))
}

fn take_plugin_string(
    s: StStr,
    plugin_free: Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)>,
) -> String {
    if s.ptr.is_null() || s.len == 0 {
        return String::new();
    }
    let text = unsafe { util::ststr_to_string_lossy(s) };
    if let Some(free) = plugin_free {
        (free)(s.ptr as *mut core::ffi::c_void, s.len, 1);
    }
    text
}

impl core::fmt::Debug for PluginLibrary {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PluginLibrary")
            .field("vtable", &self.vtable)
            .finish()
    }
}

// These types contain raw pointers and a dynamic library handle. The host keeps the library loaded
// for the process lifetime (v1), and the pointers are only used to call into that library.
unsafe impl Send for PluginLibrary {}
unsafe impl Sync for PluginLibrary {}

impl PluginLibrary {
    /// # Safety
    /// Loads and executes foreign code in-process.
    pub unsafe fn load(
        path: impl AsRef<Path>,
        entry_symbol: &str,
        host: &StHostVTableV1,
    ) -> Result<Self> {
        let path = path.as_ref();
        debug!(
            target: "stellatune_plugins::load",
            plugin_path = %path.display(),
            entry_symbol,
            "loading plugin library"
        );
        let lib = unsafe { Library::new(path) }
            .with_context(|| format!("failed to load plugin library from {}", path.display()))?;

        let entry: Symbol<StPluginEntryV1> = unsafe {
            lib.get(entry_symbol.as_bytes()).with_context(|| {
                format!(
                    "missing entry symbol `{}` in {}",
                    entry_symbol,
                    path.display()
                )
            })?
        };

        let vtable = unsafe { (entry)(host as *const StHostVTableV1) };
        if vtable.is_null() {
            return Err(anyhow!(
                "plugin `{}` returned a null vtable",
                path.display()
            ));
        }

        let api_version = unsafe { (*vtable).api_version };
        if api_version != STELLATUNE_PLUGIN_API_VERSION_V1 {
            return Err(anyhow!(
                "plugin `{}` api_version mismatch: plugin={}, host={}",
                path.display(),
                api_version,
                STELLATUNE_PLUGIN_API_VERSION_V1
            ));
        }

        debug!(
            target: "stellatune_plugins::load",
            plugin_path = %path.display(),
            api_version,
            "loaded plugin vtable"
        );
        Ok(Self { _lib: lib, vtable })
    }

    pub fn plugin_free(
        &self,
    ) -> Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)> {
        unsafe { (*self.vtable).plugin_free }
    }

    pub fn dsp_count(&self) -> usize {
        unsafe { ((*self.vtable).dsp_count)() }
    }

    pub fn dsp_get(&self, index: usize) -> *const stellatune_plugin_api::StDspVTableV1 {
        unsafe { ((*self.vtable).dsp_get)(index) }
    }

    pub fn decoder_count(&self) -> usize {
        unsafe { ((*self.vtable).decoder_count)() }
    }

    pub fn decoder_get(&self, index: usize) -> *const StDecoderVTableV1 {
        unsafe { ((*self.vtable).decoder_get)(index) }
    }

    pub fn id(&self) -> String {
        unsafe { util::ststr_to_string_lossy(((*self.vtable).id_utf8)()) }
    }

    pub fn name(&self) -> String {
        unsafe { util::ststr_to_string_lossy(((*self.vtable).name_utf8)()) }
    }

    pub fn metadata_json(&self) -> String {
        unsafe { util::ststr_to_string_lossy(((*self.vtable).metadata_json_utf8)()) }
    }

    pub fn vtable(&self) -> *const StPluginVTableV1 {
        self.vtable
    }

    pub fn get_interface_raw(&self, interface_id: &str) -> *const core::ffi::c_void {
        let Some(get_interface) = (unsafe { (*self.vtable).get_interface }) else {
            return core::ptr::null();
        };
        let id = StStr {
            ptr: interface_id.as_ptr(),
            len: interface_id.len(),
        };
        (get_interface)(id)
    }
}

struct FileIo {
    file: std::fs::File,
    size: Option<u64>,
}

impl FileIo {
    fn open(path: &str) -> Result<Self> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("failed to open file for decoder IO: {path}"))?;
        let size = file.metadata().ok().map(|m| m.len());
        Ok(Self { file, size })
    }
}

fn st_ok() -> StStatus {
    StStatus::ok()
}

fn st_err(code: i32) -> StStatus {
    StStatus {
        code,
        message: StStr::empty(),
    }
}

extern "C" fn fileio_read(
    handle: *mut core::ffi::c_void,
    out: *mut u8,
    len: usize,
    out_read: *mut usize,
) -> StStatus {
    if handle.is_null() || out.is_null() || out_read.is_null() {
        return st_err(ST_ERR_INVALID_ARG);
    }
    let io = unsafe { &mut *(handle as *mut FileIo) };
    let buf = unsafe { core::slice::from_raw_parts_mut(out, len) };
    match io.file.read(buf) {
        Ok(n) => {
            unsafe { *out_read = n };
            st_ok()
        }
        Err(_) => st_err(ST_ERR_IO),
    }
}

extern "C" fn fileio_seek(
    handle: *mut core::ffi::c_void,
    offset: i64,
    whence: StSeekWhence,
    out_pos: *mut u64,
) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return st_err(ST_ERR_INVALID_ARG);
    }
    let io = unsafe { &mut *(handle as *mut FileIo) };
    let from = match whence {
        StSeekWhence::Start => SeekFrom::Start(offset.max(0) as u64),
        StSeekWhence::Current => SeekFrom::Current(offset),
        StSeekWhence::End => SeekFrom::End(offset),
    };
    match io.file.seek(from) {
        Ok(pos) => {
            unsafe { *out_pos = pos };
            st_ok()
        }
        Err(_) => st_err(ST_ERR_IO),
    }
}

extern "C" fn fileio_tell(handle: *mut core::ffi::c_void, out_pos: *mut u64) -> StStatus {
    if handle.is_null() || out_pos.is_null() {
        return st_err(ST_ERR_INVALID_ARG);
    }
    let io = unsafe { &mut *(handle as *mut FileIo) };
    match io.file.stream_position() {
        Ok(pos) => {
            unsafe { *out_pos = pos };
            st_ok()
        }
        Err(_) => st_err(ST_ERR_IO),
    }
}

extern "C" fn fileio_size(handle: *mut core::ffi::c_void, out_size: *mut u64) -> StStatus {
    if handle.is_null() || out_size.is_null() {
        return st_err(ST_ERR_INVALID_ARG);
    }
    let io = unsafe { &mut *(handle as *mut FileIo) };
    match io.size {
        Some(n) => {
            unsafe { *out_size = n };
            st_ok()
        }
        None => st_err(ST_ERR_INTERNAL),
    }
}

static FILE_IO_VTABLE: StIoVTableV1 = StIoVTableV1 {
    read: fileio_read,
    seek: Some(fileio_seek),
    tell: Some(fileio_tell),
    size: Some(fileio_size),
};

static SHADOW_LOAD_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default)]
struct RuntimeState {
    lifecycle: LifecycleStore,
    instances: InstanceRegistry,
    next_generation_id: AtomicU64,
}

impl RuntimeState {
    fn activate_generation(&self, plugin_id: &str) -> Arc<GenerationGuard> {
        let next = self
            .next_generation_id
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        let generation = GenerationGuard::new_active(GenerationId(next));
        self.lifecycle
            .activate_generation(plugin_id, Arc::clone(&generation));
        generation
    }

    fn register_instance(
        &self,
        plugin_id: String,
        capability_type_id: String,
        kind: RuntimeCapabilityKind,
        generation: Arc<GenerationGuard>,
    ) -> InstanceId {
        self.instances
            .register(plugin_id, capability_type_id, kind, generation)
    }

    fn unregister_instance(&self, instance_id: InstanceId) {
        let _ = self.instances.remove(instance_id);
    }
}

struct GenerationCallGuard {
    generation: Arc<GenerationGuard>,
}

impl GenerationCallGuard {
    fn enter(generation: &Arc<GenerationGuard>) -> Self {
        generation.inc_inflight_call();
        Self {
            generation: Arc::clone(generation),
        }
    }
}

impl Drop for GenerationCallGuard {
    fn drop(&mut self) {
        self.generation.dec_inflight_call();
    }
}

#[derive(Debug)]
struct ShadowRootCleanup {
    root: PathBuf,
}

impl Drop for ShadowRootCleanup {
    fn drop(&mut self) {
        if self.root.as_os_str().is_empty() {
            return;
        }
        if let Err(e) = std::fs::remove_dir_all(&self.root) {
            debug!(
                target: "stellatune_plugins::shadow",
                root = %self.root.display(),
                "shadow cleanup skipped: {e:#}"
            );
        }
    }
}

#[derive(Debug)]
struct LoadedPluginRuntime {
    plugin_id: String,
    generation: Arc<GenerationGuard>,
    runtime_state: Arc<RuntimeState>,
    library: PluginLibrary,
    _host_vtable: Box<StHostVTableV1>,
    _host_ctx: Box<PluginHostCtx>,
    _shadow_cleanup: Option<ShadowRootCleanup>,
}

impl LoadedPluginRuntime {
    fn register_instance(
        &self,
        capability_type_id: String,
        kind: RuntimeCapabilityKind,
    ) -> InstanceId {
        self.runtime_state.register_instance(
            self.plugin_id.clone(),
            capability_type_id,
            kind,
            Arc::clone(&self.generation),
        )
    }

    fn unregister_instance(&self, instance_id: InstanceId) {
        self.runtime_state.unregister_instance(instance_id);
    }
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub root_dir: PathBuf,
    pub manifest: PluginManifest,
    pub library_path: PathBuf,
    runtime: Arc<LoadedPluginRuntime>,
}

impl LoadedPlugin {
    pub fn library(&self) -> &PluginLibrary {
        &self.runtime.library
    }

    fn runtime(&self) -> Arc<LoadedPluginRuntime> {
        Arc::clone(&self.runtime)
    }
}

#[derive(Debug, Clone)]
pub struct LoadedPluginInfo {
    pub id: String,
    pub name: String,
    pub root_dir: PathBuf,
    pub library_path: PathBuf,
}

#[derive(Default, Debug)]
pub struct LoadReport {
    pub loaded: Vec<LoadedPluginInfo>,
    pub errors: Vec<anyhow::Error>,
}

const HOST_TO_PLUGIN_QUEUE_CAP: usize = 1024;
const PLUGIN_TO_HOST_QUEUE_CAP: usize = 2048;

pub struct PluginManager {
    host: StHostVTableV1,
    plugins: Vec<LoadedPlugin>,
    disabled_ids: HashSet<String>,
    event_bus: PluginEventBus,
    runtime_state: Arc<RuntimeState>,
}

impl Clone for PluginManager {
    fn clone(&self) -> Self {
        Self {
            host: self.host,
            plugins: self.plugins.clone(),
            disabled_ids: self.disabled_ids.clone(),
            event_bus: self.event_bus.clone(),
            runtime_state: Arc::clone(&self.runtime_state),
        }
    }
}

unsafe impl Send for PluginManager {}
unsafe impl Sync for PluginManager {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DspKey {
    pub plugin_index: usize,
    pub dsp_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DecoderKey {
    pub plugin_index: usize,
    pub decoder_index: usize,
}

#[derive(Debug, Clone)]
pub struct DspTypeInfo {
    pub key: DspKey,
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[derive(Debug, Clone)]
pub struct DecoderTypeInfo {
    pub key: DecoderKey,
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceCatalogKey {
    pub plugin_index: usize,
    pub source_catalog_index: usize,
}

#[derive(Debug, Clone)]
pub struct SourceCatalogTypeInfo {
    pub key: SourceCatalogKey,
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LyricsProviderKey {
    pub plugin_index: usize,
}

#[derive(Debug, Clone)]
pub struct LyricsProviderTypeInfo {
    pub key: LyricsProviderKey,
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputSinkKey {
    pub plugin_index: usize,
    pub output_sink_index: usize,
}

#[derive(Debug, Clone)]
pub struct OutputSinkTypeInfo {
    pub key: OutputSinkKey,
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSinkNegotiatedSpec {
    pub spec: StAudioSpec,
    pub preferred_chunk_frames: u32,
    pub flags: u32,
}

pub struct SourceStream {
    io_vtable: *const StIoVTableV1,
    io_handle: *mut core::ffi::c_void,
    close_stream: extern "C" fn(io_handle: *mut core::ffi::c_void),
    instance_id: InstanceId,
    _plugin_runtime: Arc<LoadedPluginRuntime>,
}

unsafe impl Send for SourceStream {}

impl SourceStream {
    pub fn io_vtable(&self) -> *const StIoVTableV1 {
        self.io_vtable
    }

    pub fn io_handle(&self) -> *mut core::ffi::c_void {
        self.io_handle
    }
}

impl Drop for SourceStream {
    fn drop(&mut self) {
        if !self.io_handle.is_null() {
            let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
            (self.close_stream)(self.io_handle);
            self.io_handle = core::ptr::null_mut();
        }
        self._plugin_runtime.unregister_instance(self.instance_id);
    }
}

pub struct OutputSinkInstance {
    handle: *mut core::ffi::c_void,
    vtable: *const StOutputSinkVTableV1,
    plugin_free: Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)>,
    instance_id: InstanceId,
    _plugin_runtime: Arc<LoadedPluginRuntime>,
}

unsafe impl Send for OutputSinkInstance {}

impl OutputSinkInstance {
    pub fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> Result<u32> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("output sink instance is not initialized"));
        }
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        let channels = channels.max(1);
        if !samples.len().is_multiple_of(channels as usize) {
            return Err(anyhow!(
                "samples length {} is not divisible by channels {}",
                samples.len(),
                channels
            ));
        }
        let frames = (samples.len() / channels as usize) as u32;
        let mut out_frames_accepted = 0u32;
        let status = unsafe {
            ((*self.vtable).write_interleaved_f32)(
                self.handle,
                frames,
                channels,
                samples.as_ptr(),
                &mut out_frames_accepted,
            )
        };
        status_to_result(
            "Output sink write_interleaved_f32",
            status,
            self.plugin_free,
        )?;
        Ok(out_frames_accepted)
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("output sink instance is not initialized"));
        }
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        let Some(flush) = (unsafe { (*self.vtable).flush }) else {
            return Ok(());
        };
        let status = (flush)(self.handle);
        status_to_result("Output sink flush", status, self.plugin_free)
    }
}

impl Drop for OutputSinkInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
            unsafe { ((*self.vtable).close)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self._plugin_runtime.unregister_instance(self.instance_id);
    }
}

pub struct DspInstance {
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::StDspVTableV1,
    instance_id: InstanceId,
    _plugin_runtime: Arc<LoadedPluginRuntime>,
}

unsafe impl Send for DspInstance {}

impl DspInstance {
    pub fn process_in_place(&mut self, samples: &mut [f32], frames: u32) {
        if self.handle.is_null() || self.vtable.is_null() {
            return;
        }
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        unsafe {
            ((*self.vtable).process_interleaved_f32_in_place)(
                self.handle,
                samples.as_mut_ptr(),
                frames,
            );
        }
    }

    /// Returns bitmask of supported input channel layouts (ST_LAYOUT_* flags).
    pub fn supported_layouts(&self) -> u32 {
        if self.vtable.is_null() {
            return stellatune_plugin_api::ST_LAYOUT_STEREO;
        }
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        unsafe { ((*self.vtable).supported_layouts)() }
    }

    /// Returns the output channel count if this DSP changes the channel count.
    /// Returns 0 if the DSP preserves the input channel count (passthrough).
    pub fn output_channels(&self) -> u16 {
        if self.vtable.is_null() {
            return 0;
        }
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        unsafe { ((*self.vtable).output_channels)() }
    }

    /// Returns true if this DSP supports the given channel layout.
    pub fn supports_layout(&self, layout: u32) -> bool {
        let supported = self.supported_layouts();
        supported == stellatune_plugin_api::ST_LAYOUT_ANY || (supported & layout) != 0
    }

    pub fn set_config_json(&mut self, json: &str) -> Result<()> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("DSP instance is not initialized"));
        }
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        let s = stellatune_plugin_api::StStr {
            ptr: json.as_ptr(),
            len: json.len(),
        };
        let status = unsafe { ((*self.vtable).set_config_json_utf8)(self.handle, s) };
        if status.code != 0 {
            return Err(anyhow!("DSP set_config failed (code={})", status.code));
        }
        Ok(())
    }
}

impl Drop for DspInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
            unsafe { ((*self.vtable).drop)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self._plugin_runtime.unregister_instance(self.instance_id);
    }
}

#[allow(dead_code)]
enum DecoderIoGuard {
    File(Box<FileIo>),
    Source(SourceStream),
}

pub struct DecoderInstance {
    handle: *mut core::ffi::c_void,
    vtable: *const StDecoderVTableV1,
    info: StDecoderInfoV1,
    plugin_free: Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)>,
    instance_id: InstanceId,
    _io_guard: DecoderIoGuard,
    _plugin_runtime: Arc<LoadedPluginRuntime>,
    plugin_id: String,
    decoder_type_id: String,
}

unsafe impl Send for DecoderInstance {}

impl DecoderInstance {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn decoder_type_id(&self) -> &str {
        &self.decoder_type_id
    }

    pub fn info(&self) -> StDecoderInfoV1 {
        self.info
    }

    pub fn spec(&self) -> StAudioSpec {
        self.info.spec
    }

    pub fn duration_ms(&self) -> Option<u64> {
        if (self.info.flags & stellatune_plugin_api::ST_DECODER_INFO_FLAG_HAS_DURATION) != 0 {
            Some(self.info.duration_ms)
        } else {
            None
        }
    }

    pub fn metadata_json(&mut self) -> Result<Option<String>> {
        let Some(get) = (unsafe { (*self.vtable).get_metadata_json_utf8 }) else {
            return Ok(None);
        };
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        let mut s = StStr::empty();
        let status = (get)(self.handle, &mut s);
        status_to_result("Decoder get_metadata_json", status, self.plugin_free)?;
        if s.ptr.is_null() || s.len == 0 {
            return Ok(None);
        }
        let text = take_plugin_string(s, self.plugin_free);
        Ok(Some(text))
    }

    pub fn metadata<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        let Some(raw) = self.metadata_json()? else {
            return Ok(None);
        };
        from_json_str(&raw, "decoder metadata").map(Some)
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<()> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("Decoder instance is not initialized"));
        }
        let Some(seek) = (unsafe { (*self.vtable).seek_ms }) else {
            return Err(anyhow!("Decoder seek not supported"));
        };
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        let status = (seek)(self.handle, position_ms);
        status_to_result("Decoder seek", status, self.plugin_free)
    }

    pub fn read_interleaved_f32(&mut self, frames: u32) -> Result<(Vec<f32>, bool)> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("Decoder instance is not initialized"));
        }
        let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
        let channels = self.info.spec.channels.max(1) as usize;
        let mut out = vec![0.0f32; (frames as usize).saturating_mul(channels)];
        let mut frames_read: u32 = 0;
        let mut eof: bool = false;
        let status = unsafe {
            ((*self.vtable).read_interleaved_f32)(
                self.handle,
                frames,
                out.as_mut_ptr(),
                &mut frames_read,
                &mut eof,
            )
        };
        status_to_result("Decoder read", status, self.plugin_free)?;
        out.truncate((frames_read as usize).saturating_mul(channels));
        Ok((out, eof))
    }
}

impl Drop for DecoderInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() && !self.vtable.is_null() {
            let _call_guard = GenerationCallGuard::enter(&self._plugin_runtime.generation);
            unsafe { ((*self.vtable).close)(self.handle) };
            self.handle = core::ptr::null_mut();
        }
        self._plugin_runtime.unregister_instance(self.instance_id);
    }
}

impl PluginManager {
    pub fn new(host: StHostVTableV1) -> Self {
        Self {
            host,
            plugins: Vec::new(),
            disabled_ids: HashSet::new(),
            event_bus: shared_plugin_event_bus(),
            runtime_state: Arc::new(RuntimeState::default()),
        }
    }

    pub fn set_disabled_ids(&mut self, disabled_ids: HashSet<String>) {
        self.disabled_ids = disabled_ids;
    }

    fn is_disabled(&self, id: &str) -> bool {
        self.disabled_ids.contains(id)
    }

    pub fn plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }

    pub fn is_plugin_enabled(&self, plugin_id: &str) -> bool {
        self.plugins.iter().any(|p| p.manifest.id == plugin_id) && !self.is_disabled(plugin_id)
    }

    pub fn push_host_event_json(&self, plugin_id: &str, event_json: &str) -> Result<()> {
        let exists = self
            .plugins
            .iter()
            .any(|p| p.manifest.id == plugin_id && !self.is_disabled(&p.manifest.id));
        if !exists {
            return Err(anyhow!("plugin not found or disabled: {plugin_id}"));
        }
        self.event_bus
            .push_host_event(plugin_id, event_json.to_string());
        Ok(())
    }

    pub fn broadcast_host_event_json(&self, event_json: &str) {
        for p in &self.plugins {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            self.event_bus
                .push_host_event(&p.manifest.id, event_json.to_string());
        }
    }

    pub fn drain_runtime_events(&self, max: usize) -> Vec<PluginRuntimeEvent> {
        self.event_bus.drain_plugin_events(max)
    }

    /// Returns currently tracked live instance ids for a plugin generation set.
    pub fn runtime_instance_ids_for_plugin(&self, plugin_id: &str) -> Vec<InstanceId> {
        self.runtime_state.instances.list_ids_for_plugin(plugin_id)
    }

    /// Collect generations that are safe to unload for the plugin id.
    pub fn runtime_collect_ready_for_unload(&self, plugin_id: &str) -> Vec<GenerationId> {
        self.runtime_state
            .lifecycle
            .collect_ready_for_unload(plugin_id)
            .into_iter()
            .map(|g| g.id())
            .collect()
    }

    pub fn list_dsp_types(&self) -> Vec<DspTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library().vtable();
            if pv.is_null() {
                continue;
            }
            let plugin_id = p.library().id();
            let plugin_name = p.library().name();
            let count = unsafe { ((*pv).dsp_count)() };
            for dsp_index in 0..count {
                let vt = unsafe { ((*pv).dsp_get)(dsp_index) };
                if vt.is_null() {
                    continue;
                }
                let type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                let display_name =
                    unsafe { util::ststr_to_string_lossy(((*vt).display_name_utf8)()) };
                let config_schema_json =
                    unsafe { util::ststr_to_string_lossy(((*vt).config_schema_json_utf8)()) };
                let default_config_json =
                    unsafe { util::ststr_to_string_lossy(((*vt).default_config_json_utf8)()) };

                out.push(DspTypeInfo {
                    key: DspKey {
                        plugin_index,
                        dsp_index,
                    },
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id,
                    display_name,
                    config_schema_json,
                    default_config_json,
                });
            }
        }
        out
    }

    pub fn list_decoder_types(&self) -> Vec<DecoderTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library().vtable();
            if pv.is_null() {
                continue;
            }
            let plugin_id = p.library().id();
            let plugin_name = p.library().name();
            let count = unsafe { ((*pv).decoder_count)() };
            for decoder_index in 0..count {
                let vt = unsafe { ((*pv).decoder_get)(decoder_index) };
                if vt.is_null() {
                    continue;
                }
                let type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                out.push(DecoderTypeInfo {
                    key: DecoderKey {
                        plugin_index,
                        decoder_index,
                    },
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id,
                });
            }
        }
        out
    }

    fn source_catalog_registry_for_plugin(
        &self,
        plugin: &LoadedPlugin,
    ) -> *const StSourceCatalogRegistryV1 {
        plugin
            .library()
            .get_interface_raw(ST_INTERFACE_SOURCE_CATALOGS_V1)
            as *const StSourceCatalogRegistryV1
    }

    fn output_sink_registry_for_plugin(
        &self,
        plugin: &LoadedPlugin,
    ) -> *const StOutputSinkRegistryV1 {
        plugin
            .library()
            .get_interface_raw(ST_INTERFACE_OUTPUT_SINKS_V1)
            as *const StOutputSinkRegistryV1
    }

    pub fn list_source_catalog_types(&self) -> Vec<SourceCatalogTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let registry = self.source_catalog_registry_for_plugin(p);
            if registry.is_null() {
                continue;
            }
            let plugin_id = p.library().id();
            let plugin_name = p.library().name();
            let count = unsafe { ((*registry).source_catalog_count)() };
            for source_catalog_index in 0..count {
                let vt = unsafe { ((*registry).source_catalog_get)(source_catalog_index) };
                if vt.is_null() {
                    continue;
                }
                let type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                let display_name =
                    unsafe { util::ststr_to_string_lossy(((*vt).display_name_utf8)()) };
                let config_schema_json =
                    unsafe { util::ststr_to_string_lossy(((*vt).config_schema_json_utf8)()) };
                let default_config_json =
                    unsafe { util::ststr_to_string_lossy(((*vt).default_config_json_utf8)()) };
                out.push(SourceCatalogTypeInfo {
                    key: SourceCatalogKey {
                        plugin_index,
                        source_catalog_index,
                    },
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id,
                    display_name,
                    config_schema_json,
                    default_config_json,
                });
            }
        }
        out
    }

    pub fn list_lyrics_provider_types(&self) -> Vec<LyricsProviderTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let vt = p
                .library()
                .get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
                as *const StLyricsProviderVTableV1;
            if vt.is_null() {
                continue;
            }
            let plugin_id = p.library().id();
            let plugin_name = p.library().name();
            let type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
            let display_name = unsafe { util::ststr_to_string_lossy(((*vt).display_name_utf8)()) };
            out.push(LyricsProviderTypeInfo {
                key: LyricsProviderKey { plugin_index },
                plugin_id,
                plugin_name,
                type_id,
                display_name,
            });
        }
        out
    }

    pub fn list_output_sink_types(&self) -> Vec<OutputSinkTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let registry = self.output_sink_registry_for_plugin(p);
            if registry.is_null() {
                continue;
            }
            let plugin_id = p.library().id();
            let plugin_name = p.library().name();
            let count = unsafe { ((*registry).output_sink_count)() };
            for output_sink_index in 0..count {
                let vt = unsafe { ((*registry).output_sink_get)(output_sink_index) };
                if vt.is_null() {
                    continue;
                }
                let type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                let display_name =
                    unsafe { util::ststr_to_string_lossy(((*vt).display_name_utf8)()) };
                let config_schema_json =
                    unsafe { util::ststr_to_string_lossy(((*vt).config_schema_json_utf8)()) };
                let default_config_json =
                    unsafe { util::ststr_to_string_lossy(((*vt).default_config_json_utf8)()) };
                out.push(OutputSinkTypeInfo {
                    key: OutputSinkKey {
                        plugin_index,
                        output_sink_index,
                    },
                    plugin_id: plugin_id.clone(),
                    plugin_name: plugin_name.clone(),
                    type_id,
                    display_name,
                    config_schema_json,
                    default_config_json,
                });
            }
        }
        out
    }

    pub fn find_source_catalog_key(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Option<SourceCatalogKey> {
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if p.library().id() != plugin_id || self.is_disabled(&p.manifest.id) {
                continue;
            }
            let registry = self.source_catalog_registry_for_plugin(p);
            if registry.is_null() {
                continue;
            }
            let count = unsafe { ((*registry).source_catalog_count)() };
            for source_catalog_index in 0..count {
                let vt = unsafe { ((*registry).source_catalog_get)(source_catalog_index) };
                if vt.is_null() {
                    continue;
                }
                let tid = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                if tid == type_id {
                    return Some(SourceCatalogKey {
                        plugin_index,
                        source_catalog_index,
                    });
                }
            }
        }
        None
    }

    pub fn find_lyrics_provider_key(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Option<LyricsProviderKey> {
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if p.library().id() != plugin_id || self.is_disabled(&p.manifest.id) {
                continue;
            }
            let vt = p
                .library()
                .get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
                as *const StLyricsProviderVTableV1;
            if vt.is_null() {
                continue;
            }
            let tid = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
            if tid == type_id {
                return Some(LyricsProviderKey { plugin_index });
            }
        }
        None
    }

    pub fn find_output_sink_key(&self, plugin_id: &str, type_id: &str) -> Option<OutputSinkKey> {
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if p.library().id() != plugin_id || self.is_disabled(&p.manifest.id) {
                continue;
            }
            let registry = self.output_sink_registry_for_plugin(p);
            if registry.is_null() {
                continue;
            }
            let count = unsafe { ((*registry).output_sink_count)() };
            for output_sink_index in 0..count {
                let vt = unsafe { ((*registry).output_sink_get)(output_sink_index) };
                if vt.is_null() {
                    continue;
                }
                let tid = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                if tid == type_id {
                    return Some(OutputSinkKey {
                        plugin_index,
                        output_sink_index,
                    });
                }
            }
        }
        None
    }

    pub fn source_catalog_vtable(
        &self,
        key: SourceCatalogKey,
    ) -> Result<*const StSourceCatalogVTableV1> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let registry = self.source_catalog_registry_for_plugin(p);
        if registry.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide source catalog interfaces",
                p.manifest.id
            ));
        }
        let count = unsafe { ((*registry).source_catalog_count)() };
        if key.source_catalog_index >= count {
            return Err(anyhow!(
                "invalid source_catalog_index {} for plugin `{}`",
                key.source_catalog_index,
                p.manifest.id
            ));
        }
        let vt = unsafe { ((*registry).source_catalog_get)(key.source_catalog_index) };
        if vt.is_null() {
            return Err(anyhow!(
                "source catalog index {} resolved to null vtable for plugin `{}`",
                key.source_catalog_index,
                p.manifest.id
            ));
        }
        Ok(vt)
    }

    pub fn lyrics_provider_vtable(
        &self,
        key: LyricsProviderKey,
    ) -> Result<*const StLyricsProviderVTableV1> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt = p
            .library()
            .get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
            as *const StLyricsProviderVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide lyrics provider interface",
                p.manifest.id
            ));
        }
        Ok(vt)
    }

    pub fn output_sink_vtable(&self, key: OutputSinkKey) -> Result<*const StOutputSinkVTableV1> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let registry = self.output_sink_registry_for_plugin(p);
        if registry.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide output sink interfaces",
                p.manifest.id
            ));
        }
        let count = unsafe { ((*registry).output_sink_count)() };
        if key.output_sink_index >= count {
            return Err(anyhow!(
                "invalid output_sink_index {} for plugin `{}`",
                key.output_sink_index,
                p.manifest.id
            ));
        }
        let vt = unsafe { ((*registry).output_sink_get)(key.output_sink_index) };
        if vt.is_null() {
            return Err(anyhow!(
                "output sink index {} resolved to null vtable for plugin `{}`",
                key.output_sink_index,
                p.manifest.id
            ));
        }
        Ok(vt)
    }

    pub fn source_list_items<C, R, Items>(
        &self,
        key: SourceCatalogKey,
        config: &C,
        request: &R,
    ) -> Result<Items>
    where
        C: Serialize,
        R: Serialize,
        Items: DeserializeOwned,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let config_json = to_json_string(config, "source config")?;
        let request_json = to_json_string(request, "source list request")?;
        let vt = self.source_catalog_vtable(key)?;
        let plugin_free = p.library().plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let req = StStr {
            ptr: request_json.as_ptr(),
            len: request_json.len(),
        };
        let mut out = StStr::empty();
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).list_items_json_utf8)(cfg, req, &mut out) };
        status_to_result("Source list_items_json", status, plugin_free)?;
        let raw = take_plugin_string(out, plugin_free);
        from_json_str(&raw, "source list response")
    }

    pub fn source_open_stream<C, T, M>(
        &self,
        key: SourceCatalogKey,
        config: &C,
        track: &T,
    ) -> Result<(SourceStream, Option<M>)>
    where
        C: Serialize,
        T: Serialize,
        M: DeserializeOwned,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let config_json = to_json_string(config, "source config")?;
        let track_json = to_json_string(track, "source track request")?;
        let vt = self.source_catalog_vtable(key)?;
        let plugin_free = p.library().plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let track = StStr {
            ptr: track_json.as_ptr(),
            len: track_json.len(),
        };
        let mut out_io_vtable: *const StIoVTableV1 = core::ptr::null();
        let mut out_io_handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let mut out_track_meta_json_utf8 = StStr::empty();
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe {
            ((*vt).open_stream)(
                cfg,
                track,
                &mut out_io_vtable,
                &mut out_io_handle,
                &mut out_track_meta_json_utf8,
            )
        };
        status_to_result("Source open_stream", status, plugin_free)?;
        if out_io_vtable.is_null() || out_io_handle.is_null() {
            if !out_io_handle.is_null() {
                unsafe { ((*vt).close_stream)(out_io_handle) };
            }
            return Err(anyhow!("source open_stream returned invalid io handle"));
        }
        let source_type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
        let instance_id =
            runtime.register_instance(source_type_id, RuntimeCapabilityKind::SourceCatalog);
        let meta = take_plugin_string(out_track_meta_json_utf8, plugin_free);
        Ok((
            SourceStream {
                io_vtable: out_io_vtable,
                io_handle: out_io_handle,
                close_stream: unsafe { (*vt).close_stream },
                instance_id,
                _plugin_runtime: runtime,
            },
            if meta.is_empty() {
                None
            } else {
                Some(from_json_str(&meta, "source track metadata")?)
            },
        ))
    }

    pub fn lyrics_search<Q, Resp>(&self, key: LyricsProviderKey, query: &Q) -> Result<Resp>
    where
        Q: Serialize,
        Resp: DeserializeOwned,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt = p
            .library()
            .get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
            as *const StLyricsProviderVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide lyrics provider interface",
                p.manifest.id
            ));
        }
        let query_json = to_json_string(query, "lyrics search query")?;
        let plugin_free = p.library().plugin_free();
        let query = StStr {
            ptr: query_json.as_ptr(),
            len: query_json.len(),
        };
        let mut out = StStr::empty();
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).search_json_utf8)(query, &mut out) };
        status_to_result("Lyrics search_json", status, plugin_free)?;
        let raw = take_plugin_string(out, plugin_free);
        from_json_str(&raw, "lyrics search response")
    }

    pub fn lyrics_fetch<T, Resp>(&self, key: LyricsProviderKey, track: &T) -> Result<Resp>
    where
        T: Serialize,
        Resp: DeserializeOwned,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt = p
            .library()
            .get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
            as *const StLyricsProviderVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide lyrics provider interface",
                p.manifest.id
            ));
        }
        let track_json = to_json_string(track, "lyrics fetch request")?;
        let plugin_free = p.library().plugin_free();
        let track = StStr {
            ptr: track_json.as_ptr(),
            len: track_json.len(),
        };
        let mut out = StStr::empty();
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).fetch_json_utf8)(track, &mut out) };
        status_to_result("Lyrics fetch_json", status, plugin_free)?;
        let raw = take_plugin_string(out, plugin_free);
        from_json_str(&raw, "lyrics fetch response")
    }

    pub fn output_list_targets<C, Targets>(&self, key: OutputSinkKey, config: &C) -> Result<Targets>
    where
        C: Serialize,
        Targets: DeserializeOwned,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let config_json = to_json_string(config, "output sink config")?;
        let vt = self.output_sink_vtable(key)?;
        let plugin_free = p.library().plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let mut out = StStr::empty();
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).list_targets_json_utf8)(cfg, &mut out) };
        status_to_result("Output sink list_targets_json", status, plugin_free)?;
        let raw = take_plugin_string(out, plugin_free);
        from_json_str(&raw, "output sink targets")
    }

    pub fn output_negotiate<C, T>(
        &self,
        key: OutputSinkKey,
        config: &C,
        target: &T,
        desired_spec: StAudioSpec,
    ) -> Result<OutputSinkNegotiatedSpec>
    where
        C: Serialize,
        T: Serialize,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let config_json = to_json_string(config, "output sink config")?;
        let target_json = to_json_string(target, "output sink target")?;
        let vt = self.output_sink_vtable(key)?;
        let plugin_free = p.library().plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let target = StStr {
            ptr: target_json.as_ptr(),
            len: target_json.len(),
        };
        let mut out = StOutputSinkNegotiatedSpecV1 {
            spec: desired_spec,
            preferred_chunk_frames: 0,
            flags: 0,
            reserved: 0,
        };
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).negotiate_spec)(cfg, target, desired_spec, &mut out) };
        status_to_result("Output sink negotiate_spec", status, plugin_free)?;
        if out.spec.sample_rate == 0 || out.spec.channels == 0 {
            return Err(anyhow!(
                "output sink negotiate_spec returned invalid spec: {}Hz {}ch",
                out.spec.sample_rate,
                out.spec.channels
            ));
        }

        let mut flags = out.flags;
        let sr_changed = out.spec.sample_rate != desired_spec.sample_rate;
        let ch_changed = out.spec.channels != desired_spec.channels;
        if sr_changed {
            flags |= ST_OUTPUT_NEGOTIATE_CHANGED_SR;
        }
        if ch_changed {
            flags |= ST_OUTPUT_NEGOTIATE_CHANGED_CH;
        }
        if !sr_changed && !ch_changed {
            flags |= ST_OUTPUT_NEGOTIATE_EXACT;
        }

        Ok(OutputSinkNegotiatedSpec {
            spec: StAudioSpec {
                sample_rate: out.spec.sample_rate.max(1),
                channels: out.spec.channels.max(1),
                reserved: 0,
            },
            preferred_chunk_frames: out.preferred_chunk_frames,
            flags,
        })
    }

    pub fn output_open<C, T>(
        &self,
        key: OutputSinkKey,
        config: &C,
        target: &T,
        spec: StAudioSpec,
    ) -> Result<OutputSinkInstance>
    where
        C: Serialize,
        T: Serialize,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let config_json = to_json_string(config, "output sink config")?;
        let target_json = to_json_string(target, "output sink target")?;
        let vt = self.output_sink_vtable(key)?;
        let plugin_free = p.library().plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let target = StStr {
            ptr: target_json.as_ptr(),
            len: target_json.len(),
        };
        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).open)(cfg, target, spec, &mut handle) };
        status_to_result("Output sink open", status, plugin_free)?;
        if handle.is_null() {
            return Err(anyhow!("output sink open returned null handle"));
        }
        let output_type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
        let instance_id =
            runtime.register_instance(output_type_id, RuntimeCapabilityKind::OutputSink);
        Ok(OutputSinkInstance {
            handle,
            vtable: vt,
            plugin_free,
            instance_id,
            _plugin_runtime: runtime,
        })
    }

    pub fn find_dsp_key(&self, plugin_id: &str, type_id: &str) -> Option<DspKey> {
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if p.library().id() != plugin_id {
                continue;
            }
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library().vtable();
            if pv.is_null() {
                continue;
            }
            let count = unsafe { ((*pv).dsp_count)() };
            for dsp_index in 0..count {
                let vt = unsafe { ((*pv).dsp_get)(dsp_index) };
                if vt.is_null() {
                    continue;
                }
                let tid = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                if tid == type_id {
                    return Some(DspKey {
                        plugin_index,
                        dsp_index,
                    });
                }
            }
        }
        None
    }

    pub fn find_decoder_key(&self, plugin_id: &str, type_id: &str) -> Option<DecoderKey> {
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if p.library().id() != plugin_id {
                continue;
            }
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library().vtable();
            if pv.is_null() {
                continue;
            }
            let count = unsafe { ((*pv).decoder_count)() };
            for decoder_index in 0..count {
                let vt = unsafe { ((*pv).decoder_get)(decoder_index) };
                if vt.is_null() {
                    continue;
                }
                let tid = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
                if tid == type_id {
                    return Some(DecoderKey {
                        plugin_index,
                        decoder_index,
                    });
                }
            }
        }
        None
    }

    fn open_decoder_with_io_guard(
        &self,
        key: DecoderKey,
        path_hint: &str,
        ext_hint: &str,
        io_vtable: *const StIoVTableV1,
        io_handle: *mut core::ffi::c_void,
        io_guard: DecoderIoGuard,
    ) -> Result<DecoderInstance> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let pv = p.library().vtable();
        if pv.is_null() {
            return Err(anyhow!("plugin has null vtable"));
        }
        let vt = unsafe { ((*pv).decoder_get)(key.decoder_index) };
        if vt.is_null() {
            return Err(anyhow!("invalid decoder_index {}", key.decoder_index));
        }
        let decoder_type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
        let plugin_id = p.library().id();
        let args = StDecoderOpenArgsV1 {
            path_utf8: StStr {
                ptr: path_hint.as_ptr(),
                len: path_hint.len(),
            },
            ext_utf8: StStr {
                ptr: ext_hint.as_ptr(),
                len: ext_hint.len(),
            },
            io_vtable,
            io_handle,
        };

        let plugin_free = p.library().plugin_free();
        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let runtime = p.runtime();
        let _open_call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).open)(args, &mut handle) };
        if status.code != 0 || handle.is_null() {
            return Err(status_err_to_anyhow("Decoder open", status, plugin_free));
        }

        let mut info = StDecoderInfoV1 {
            spec: StAudioSpec {
                sample_rate: 0,
                channels: 0,
                reserved: 0,
            },
            duration_ms: 0,
            flags: 0,
            reserved: 0,
        };
        let _get_info_call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).get_info)(handle, &mut info) };
        if status.code != 0 {
            unsafe { ((*vt).close)(handle) };
            return Err(status_err_to_anyhow(
                "Decoder get_info",
                status,
                plugin_free,
            ));
        }

        let instance_id =
            runtime.register_instance(decoder_type_id.clone(), RuntimeCapabilityKind::Decoder);

        Ok(DecoderInstance {
            handle,
            vtable: vt,
            info,
            plugin_free,
            instance_id,
            _io_guard: io_guard,
            _plugin_runtime: runtime,
            plugin_id,
            decoder_type_id,
        })
    }

    pub fn open_decoder(&self, key: DecoderKey, path: &str) -> Result<DecoderInstance> {
        let mut io = Box::new(FileIo::open(path)?);
        let io_handle = (&mut *io) as *mut FileIo as *mut core::ffi::c_void;
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        self.open_decoder_with_io_guard(
            key,
            path,
            &ext,
            &FILE_IO_VTABLE as *const StIoVTableV1,
            io_handle,
            DecoderIoGuard::File(io),
        )
    }

    pub fn open_decoder_with_source_stream(
        &self,
        key: DecoderKey,
        path_hint: &str,
        ext_hint: &str,
        stream: SourceStream,
    ) -> Result<DecoderInstance> {
        let io_vtable = stream.io_vtable();
        let io_handle = stream.io_handle();
        self.open_decoder_with_io_guard(
            key,
            path_hint,
            ext_hint,
            io_vtable,
            io_handle,
            DecoderIoGuard::Source(stream),
        )
    }

    pub fn open_best_decoder(&self, path: &str) -> Result<Option<DecoderInstance>> {
        let Some((key, score)) = self.probe_best_decoder(path)? else {
            return Ok(None);
        };

        if let Some(p) = self.plugins.get(key.plugin_index) {
            let pv = p.library().vtable();
            let decoder_type_id = if pv.is_null() {
                "<unknown>".to_string()
            } else {
                let vt = unsafe { ((*pv).decoder_get)(key.decoder_index) };
                if vt.is_null() {
                    "<unknown>".to_string()
                } else {
                    unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) }
                }
            };
            debug!(
                target: "stellatune_plugins::decode",
                path,
                ext = %std::path::Path::new(path).extension().and_then(|s| s.to_str()).unwrap_or(""),
                plugin_id = %p.library().id(),
                decoder_type_id = %decoder_type_id,
                score,
                "selected plugin decoder"
            );
        }
        Ok(Some(self.open_decoder(key, path)?))
    }

    /// Returns the best decoder key + score for the given path, without opening the decoder.
    pub fn probe_best_decoder(&self, path: &str) -> Result<Option<(DecoderKey, u8)>> {
        use std::io::Read;

        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let mut header = Vec::new();
        if let Ok(mut f) = std::fs::File::open(path) {
            let mut buf = vec![0u8; 64 * 1024];
            if let Ok(n) = f.read(&mut buf) {
                buf.truncate(n);
                header = buf;
            }
        }

        let ext_str = stellatune_plugin_api::StStr {
            ptr: ext.as_ptr(),
            len: ext.len(),
        };
        let header_slice = stellatune_plugin_api::StSlice::<u8> {
            ptr: header.as_ptr(),
            len: header.len(),
        };

        let mut best: Option<(DecoderKey, u8)> = None;
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library().vtable();
            if pv.is_null() {
                continue;
            }
            let count = unsafe { ((*pv).decoder_count)() };
            for decoder_index in 0..count {
                let vt = unsafe { ((*pv).decoder_get)(decoder_index) };
                if vt.is_null() {
                    continue;
                }
                let score = unsafe { ((*vt).probe)(ext_str, header_slice) };
                if score == 0 {
                    continue;
                }
                match best {
                    None => {
                        best = Some((
                            DecoderKey {
                                plugin_index,
                                decoder_index,
                            },
                            score,
                        ))
                    }
                    Some((_, best_score)) if score > best_score => {
                        best = Some((
                            DecoderKey {
                                plugin_index,
                                decoder_index,
                            },
                            score,
                        ))
                    }
                    _ => {}
                }
            }
        }

        if let Some((key, score)) = best {
            let p = &self.plugins[key.plugin_index];
            let pv = p.library().vtable();
            let decoder_type_id = if pv.is_null() {
                "<unknown>".to_string()
            } else {
                let vt = unsafe { ((*pv).decoder_get)(key.decoder_index) };
                if vt.is_null() {
                    "<unknown>".to_string()
                } else {
                    unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) }
                }
            };
            debug!(
                target: "stellatune_plugins::decode",
                path,
                ext = %ext,
                header_len = header.len(),
                plugin_id = %p.library().id(),
                decoder_type_id = %decoder_type_id,
                score,
                "best plugin decoder probe match"
            );
            return Ok(Some((key, score)));
        }

        Ok(None)
    }

    /// Returns the best decoder key + score for an extension hint, without reading the file.
    ///
    /// This passes an empty header slice to `probe`. Decoders should treat the header as optional
    /// and may return a non-zero score based on extension alone.
    pub fn probe_best_decoder_hint(&self, path_ext: &str) -> Option<(DecoderKey, u8)> {
        let ext_str = stellatune_plugin_api::StStr {
            ptr: path_ext.as_ptr(),
            len: path_ext.len(),
        };
        let header_slice = stellatune_plugin_api::StSlice::<u8> {
            ptr: core::ptr::null(),
            len: 0,
        };

        let mut best: Option<(DecoderKey, u8)> = None;
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library().vtable();
            if pv.is_null() {
                continue;
            }
            let count = unsafe { ((*pv).decoder_count)() };
            for decoder_index in 0..count {
                let vt = unsafe { ((*pv).decoder_get)(decoder_index) };
                if vt.is_null() {
                    continue;
                }
                let score = unsafe { ((*vt).probe)(ext_str, header_slice) };
                if score == 0 {
                    continue;
                }
                match best {
                    None => {
                        best = Some((
                            DecoderKey {
                                plugin_index,
                                decoder_index,
                            },
                            score,
                        ))
                    }
                    Some((_, best_score)) if score > best_score => {
                        best = Some((
                            DecoderKey {
                                plugin_index,
                                decoder_index,
                            },
                            score,
                        ))
                    }
                    _ => {}
                }
            }
        }

        best
    }

    pub fn can_decode_path(&self, path: &str) -> Result<bool> {
        Ok(self.probe_best_decoder(path)?.is_some())
    }

    pub fn create_dsp<C>(
        &self,
        key: DspKey,
        sample_rate: u32,
        channels: u16,
        config: &C,
    ) -> Result<DspInstance>
    where
        C: Serialize,
    {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let pv = p.library().vtable();
        if pv.is_null() {
            return Err(anyhow!("plugin has null vtable"));
        }
        let vt = unsafe { ((*pv).dsp_get)(key.dsp_index) };
        if vt.is_null() {
            return Err(anyhow!("invalid dsp_index {}", key.dsp_index));
        }

        let config_json = to_json_string(config, "dsp config")?;
        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let json = stellatune_plugin_api::StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let runtime = p.runtime();
        let _call_guard = GenerationCallGuard::enter(&runtime.generation);
        let status = unsafe { ((*vt).create)(sample_rate, channels, json, &mut handle) };
        if status.code != 0 || handle.is_null() {
            return Err(anyhow!("DSP create failed (code={})", status.code));
        }
        let dsp_type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
        let instance_id = runtime.register_instance(dsp_type_id, RuntimeCapabilityKind::Dsp);

        Ok(DspInstance {
            handle,
            vtable: vt,
            instance_id,
            _plugin_runtime: runtime,
        })
    }

    /// # Safety
    /// Loads and executes foreign code in-process.
    pub unsafe fn load_dir(&mut self, dir: impl AsRef<Path>) -> Result<LoadReport> {
        unsafe { self.load_dir_filtered(dir, &std::collections::HashSet::new()) }
    }

    /// # Safety
    /// Loads and executes foreign code in-process.
    pub unsafe fn load_dir_filtered(
        &mut self,
        dir: impl AsRef<Path>,
        disabled_ids: &std::collections::HashSet<String>,
    ) -> Result<LoadReport> {
        let dir = dir.as_ref();
        info!(
            target: "stellatune_plugins::load",
            plugin_dir = %dir.display(),
            disabled = disabled_ids.len(),
            "discovering plugins"
        );
        let mut report = LoadReport::default();
        for discovered in manifest::discover_plugins(dir)? {
            if disabled_ids.contains(&discovered.manifest.id) {
                debug!(
                    target: "stellatune_plugins::load",
                    plugin_id = %discovered.manifest.id,
                    "skipping disabled plugin"
                );
                continue;
            }
            match unsafe { self.load_discovered(&discovered) }
                .with_context(|| format!("while loading plugin `{}`", discovered.manifest.id))
            {
                Ok(loaded) => {
                    info!(
                        target: "stellatune_plugins::load",
                        plugin_id = %loaded.library().id(),
                        plugin_name = %loaded.library().name(),
                        root_dir = %loaded.root_dir.display(),
                        library_path = %loaded.library_path.display(),
                        decoders = loaded.library().decoder_count(),
                        dsps = loaded.library().dsp_count(),
                        "plugin loaded"
                    );
                    report.loaded.push(LoadedPluginInfo {
                        id: loaded.library().id(),
                        name: loaded.library().name(),
                        root_dir: loaded.root_dir.clone(),
                        library_path: loaded.library_path.clone(),
                    });
                    self.plugins.push(loaded);
                }
                Err(e) => {
                    warn!(
                        target: "stellatune_plugins::load",
                        plugin_id = %discovered.manifest.id,
                        "plugin load failed: {e:#}"
                    );
                    report.errors.push(e)
                }
            }
        }
        Ok(report)
    }

    /// # Safety
    /// Loads and executes foreign code in-process.
    ///
    /// Unlike `load_dir_filtered`, this will *skip* plugins that are already loaded (by manifest id).
    pub unsafe fn load_dir_additive_filtered(
        &mut self,
        dir: impl AsRef<Path>,
        disabled_ids: &std::collections::HashSet<String>,
    ) -> Result<LoadReport> {
        let dir = dir.as_ref();
        info!(
            target: "stellatune_plugins::load",
            plugin_dir = %dir.display(),
            disabled = disabled_ids.len(),
            already_loaded = self.plugins.len(),
            "discovering plugins (additive)"
        );
        let mut report = LoadReport::default();
        for discovered in manifest::discover_plugins(dir)? {
            if disabled_ids.contains(&discovered.manifest.id) {
                debug!(
                    target: "stellatune_plugins::load",
                    plugin_id = %discovered.manifest.id,
                    "skipping disabled plugin"
                );
                continue;
            }
            if self
                .plugins
                .iter()
                .any(|p| p.manifest.id == discovered.manifest.id)
            {
                debug!(
                    target: "stellatune_plugins::load",
                    plugin_id = %discovered.manifest.id,
                    "skipping already loaded plugin"
                );
                continue;
            }
            match unsafe { self.load_discovered(&discovered) }
                .with_context(|| format!("while loading plugin `{}`", discovered.manifest.id))
            {
                Ok(loaded) => {
                    info!(
                        target: "stellatune_plugins::load",
                        plugin_id = %loaded.library().id(),
                        plugin_name = %loaded.library().name(),
                        root_dir = %loaded.root_dir.display(),
                        library_path = %loaded.library_path.display(),
                        decoders = loaded.library().decoder_count(),
                        dsps = loaded.library().dsp_count(),
                        "plugin loaded"
                    );
                    report.loaded.push(LoadedPluginInfo {
                        id: loaded.library().id(),
                        name: loaded.library().name(),
                        root_dir: loaded.root_dir.clone(),
                        library_path: loaded.library_path.clone(),
                    });
                    self.plugins.push(loaded);
                }
                Err(e) => {
                    warn!(
                        target: "stellatune_plugins::load",
                        plugin_id = %discovered.manifest.id,
                        "plugin load failed: {e:#}"
                    );
                    report.errors.push(e)
                }
            }
        }
        Ok(report)
    }

    /// # Safety
    /// Loads and executes foreign code in-process.
    pub unsafe fn load_discovered(&self, discovered: &DiscoveredPlugin) -> Result<LoadedPlugin> {
        if discovered.manifest.api_version != STELLATUNE_PLUGIN_API_VERSION_V1 {
            return Err(anyhow!(
                "plugin `{}` api_version mismatch: plugin={}, host={}",
                discovered.manifest.id,
                discovered.manifest.api_version,
                STELLATUNE_PLUGIN_API_VERSION_V1
            ));
        }

        let install_library_path = discovered.library_path.clone();
        if !install_library_path.exists() {
            return Err(anyhow!(
                "plugin `{}` library not found: {}",
                discovered.manifest.id,
                install_library_path.display()
            ));
        }
        let (runtime_root_dir, load_library_path, shadow_cleanup) =
            match prepare_shadow_load_copy(discovered) {
                Ok((shadow_root, shadow_library_path)) => {
                    debug!(
                        target: "stellatune_plugins::shadow",
                        plugin_id = %discovered.manifest.id,
                        install_root = %discovered.root_dir.display(),
                        shadow_root = %shadow_root.display(),
                        shadow_library = %shadow_library_path.display(),
                        "prepared shadow load copy"
                    );
                    (
                        shadow_root.clone(),
                        shadow_library_path,
                        Some(ShadowRootCleanup { root: shadow_root }),
                    )
                }
                Err(e) => {
                    warn!(
                        target: "stellatune_plugins::shadow",
                        plugin_id = %discovered.manifest.id,
                        install_root = %discovered.root_dir.display(),
                        "shadow load unavailable; fallback to install library: {e:#}"
                    );
                    (discovered.root_dir.clone(), install_library_path, None)
                }
            };
        let runtime_root = runtime_root_dir.to_string_lossy().into_owned();
        self.event_bus.register_plugin(&discovered.manifest.id);
        let mut host_ctx = Box::new(PluginHostCtx {
            plugin_id: discovered.manifest.id.clone(),
            runtime_root_utf8: runtime_root.into_bytes().into_boxed_slice(),
            event_bus: self.event_bus.clone(),
        });
        let mut host_vtable = Box::new(self.host);
        host_vtable.user_data = (&mut *host_ctx) as *mut PluginHostCtx as *mut core::ffi::c_void;
        host_vtable.get_runtime_root_utf8 = Some(plugin_host_runtime_root);
        host_vtable.emit_event_json_utf8 = Some(plugin_host_emit_event_json);
        host_vtable.poll_host_event_json_utf8 = Some(plugin_host_poll_event_json);
        host_vtable.send_control_json_utf8 = Some(plugin_host_send_control_json);
        host_vtable.free_host_str_utf8 = Some(plugin_host_free_str);

        let library = unsafe {
            PluginLibrary::load(
                &load_library_path,
                discovered.manifest.entry_symbol(),
                host_vtable.as_ref(),
            )?
        };

        let exported_id = library.id();
        if exported_id != discovered.manifest.id {
            return Err(anyhow!(
                "plugin id mismatch: manifest.id=`{}`, exported.id=`{}`",
                discovered.manifest.id,
                exported_id
            ));
        }

        let metadata_json = library.metadata_json();
        let meta: PluginMetadata = serde_json::from_str(&metadata_json).with_context(|| {
            format!(
                "invalid metadata_json_utf8 for plugin `{}` at {}",
                exported_id,
                load_library_path.display()
            )
        })?;
        if meta.id != exported_id {
            return Err(anyhow!(
                "plugin metadata id mismatch: metadata.id=`{}`, exported.id=`{}`",
                meta.id,
                exported_id
            ));
        }
        let exported_name = library.name();
        if meta.name != exported_name {
            return Err(anyhow!(
                "plugin metadata name mismatch: metadata.name=`{}`, exported.name=`{}`",
                meta.name,
                exported_name
            ));
        }
        if meta.api_version != STELLATUNE_PLUGIN_API_VERSION_V1 {
            return Err(anyhow!(
                "plugin `{}` metadata api_version mismatch: plugin={}, host={}",
                exported_id,
                meta.api_version,
                STELLATUNE_PLUGIN_API_VERSION_V1
            ));
        }
        let generation = self
            .runtime_state
            .activate_generation(&discovered.manifest.id);

        Ok(LoadedPlugin {
            root_dir: discovered.root_dir.clone(),
            manifest: discovered.manifest.clone(),
            library_path: load_library_path,
            runtime: Arc::new(LoadedPluginRuntime {
                plugin_id: discovered.manifest.id.clone(),
                generation,
                runtime_state: Arc::clone(&self.runtime_state),
                library,
                _host_vtable: host_vtable,
                _host_ctx: host_ctx,
                _shadow_cleanup: shadow_cleanup,
            }),
        })
    }
}

fn dynamic_library_ext() -> &'static str {
    match std::env::consts::OS {
        "windows" => "dll",
        "linux" => "so",
        "macos" => "dylib",
        _ => "",
    }
}

fn is_dynamic_library_file(path: &Path) -> bool {
    let ext = dynamic_library_ext();
    if ext.is_empty() {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

fn sanitize_plugin_id_for_path(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "plugin".to_string()
    } else {
        out
    }
}

fn shadow_plugins_root() -> PathBuf {
    std::env::temp_dir()
        .join("stellatune")
        .join("plugin-shadow")
}

fn prepare_shadow_load_copy(discovered: &DiscoveredPlugin) -> Result<(PathBuf, PathBuf)> {
    let rel_library_path = discovered
        .library_path
        .strip_prefix(&discovered.root_dir)
        .with_context(|| {
            format!(
                "failed to derive plugin library relative path: root={} library={}",
                discovered.root_dir.display(),
                discovered.library_path.display()
            )
        })?;
    let shadow_base = shadow_plugins_root();
    std::fs::create_dir_all(&shadow_base)
        .with_context(|| format!("create {}", shadow_base.display()))?;
    let pid = std::process::id();
    let seq = SHADOW_LOAD_SEQ.fetch_add(1, Ordering::Relaxed);
    let shadow_root = shadow_base.join(format!(
        "{}-{}-{}-{}",
        sanitize_plugin_id_for_path(&discovered.manifest.id),
        pid,
        now_unix_ms(),
        seq
    ));
    copy_dir_recursive(&discovered.root_dir, &shadow_root)?;
    let shadow_library_path = shadow_root.join(rel_library_path);
    if !shadow_library_path.exists() {
        return Err(anyhow!(
            "shadow plugin library not found after copy: {}",
            shadow_library_path.display()
        ));
    }
    Ok((shadow_root, shadow_library_path))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("create {}", dst.display()))?;
    for entry in walkdir::WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path == src {
            continue;
        }
        let rel = path
            .strip_prefix(src)
            .with_context(|| format!("strip prefix {} from {}", src.display(), path.display()))?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)
                .with_context(|| format!("create {}", target.display()))?;
            continue;
        }
        if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            std::fs::copy(path, &target)
                .with_context(|| format!("copy {} -> {}", path.display(), target.display()))?;
        }
    }
    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, out_dir: &Path) -> Result<()> {
    let buf = std::fs::read(zip_path).with_context(|| format!("read {}", zip_path.display()))?;
    let archive = rawzip::ZipArchive::from_slice(&buf)
        .map_err(|e| anyhow!("invalid zip archive: {:?}", e))?;

    for entry in archive.entries() {
        let entry = entry.map_err(|e| anyhow!("zip entry error: {:?}", e))?;
        let filename = entry
            .file_path()
            .try_normalize()
            .map_err(|e| anyhow!("failed to normalize zip path: {:?}", e))?
            .as_ref()
            .to_string();

        // Basic zip-slip protection: check for ".." or absolute paths
        let path = Path::new(&filename);
        if path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(anyhow!(
                "unsupported or malicious path in zip: {}",
                filename
            ));
        }

        let out_path = out_dir.join(&filename);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)
                .with_context(|| format!("create {}", out_path.display()))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }

        let mut out = std::fs::File::create(&out_path)
            .with_context(|| format!("create {}", out_path.display()))?;

        let wayfinder = entry.wayfinder();
        let slice_entry = archive
            .get_entry(wayfinder)
            .map_err(|e| anyhow!("failed to get entry data: {:?}", e))?;
        let data = slice_entry.data();

        match entry.compression_method() {
            rawzip::CompressionMethod::Store => {
                std::io::copy(&mut &*data, &mut out)
                    .with_context(|| format!("extract {} to {}", filename, out_path.display()))?;
            }
            rawzip::CompressionMethod::Deflate => {
                let mut decoder = flate2::read::DeflateDecoder::new(data);
                std::io::copy(&mut decoder, &mut out).with_context(|| {
                    format!("extract (deflate) {} to {}", filename, out_path.display())
                })?;
            }
            m => return Err(anyhow!("unsupported compression method: {:?}", m)),
        }
    }
    Ok(())
}

fn inspect_plugin_library_at(path: &Path, runtime_root: &Path) -> Result<PluginManifest> {
    let event_bus = PluginEventBus::new(HOST_TO_PLUGIN_QUEUE_CAP, PLUGIN_TO_HOST_QUEUE_CAP);
    let mut host_ctx = Box::new(PluginHostCtx {
        plugin_id: "__inspect__".to_string(),
        runtime_root_utf8: runtime_root
            .to_string_lossy()
            .into_owned()
            .into_bytes()
            .into_boxed_slice(),
        event_bus,
    });
    let mut host_vtable = Box::new(default_host_vtable());
    host_vtable.user_data = (&mut *host_ctx) as *mut PluginHostCtx as *mut core::ffi::c_void;
    host_vtable.get_runtime_root_utf8 = Some(plugin_host_runtime_root);
    host_vtable.emit_event_json_utf8 = Some(plugin_host_emit_event_json);
    host_vtable.poll_host_event_json_utf8 = Some(plugin_host_poll_event_json);
    host_vtable.send_control_json_utf8 = Some(plugin_host_send_control_json);
    host_vtable.free_host_str_utf8 = Some(plugin_host_free_str);

    let library = unsafe {
        PluginLibrary::load(
            path,
            STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1,
            host_vtable.as_ref(),
        )
    }?;
    let id = library.id();
    let name = library.name();
    let metadata_json = library.metadata_json();
    let meta: PluginMetadata = serde_json::from_str(&metadata_json).with_context(|| {
        format!(
            "invalid metadata_json_utf8 for plugin `{}` at {}",
            id,
            path.display()
        )
    })?;
    if meta.id != id {
        return Err(anyhow!(
            "plugin metadata id mismatch: metadata.id=`{}`, exported.id=`{}`",
            meta.id,
            id
        ));
    }
    if meta.name != name {
        return Err(anyhow!(
            "plugin metadata name mismatch: metadata.name=`{}`, exported.name=`{}`",
            meta.name,
            name
        ));
    }
    if meta.api_version != STELLATUNE_PLUGIN_API_VERSION_V1 {
        return Err(anyhow!(
            "plugin `{}` metadata api_version mismatch: plugin={}, host={}",
            id,
            meta.api_version,
            STELLATUNE_PLUGIN_API_VERSION_V1
        ));
    }

    Ok(PluginManifest {
        id,
        api_version: meta.api_version,
        name: Some(name),
        entry_symbol: Some(STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1.to_string()),
        metadata: Some(meta),
    })
}

fn find_plugin_library_candidates(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(8)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if is_dynamic_library_file(path) {
            out.push(path.to_path_buf());
        }
    }
    out
}

pub fn install_plugin_from_artifact(
    plugins_dir: impl AsRef<Path>,
    artifact_path: impl AsRef<Path>,
) -> Result<InstalledPluginInfo> {
    let plugins_dir = plugins_dir.as_ref();
    let artifact_path = artifact_path.as_ref();
    if !artifact_path.exists() {
        return Err(anyhow!("artifact not found: {}", artifact_path.display()));
    }
    std::fs::create_dir_all(plugins_dir)
        .with_context(|| format!("create {}", plugins_dir.display()))?;

    let temp = tempfile::tempdir().context("create plugin install temp dir")?;
    let staging_root = temp.path().join("staging");
    std::fs::create_dir_all(&staging_root)
        .with_context(|| format!("create {}", staging_root.display()))?;

    let ext = artifact_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if ext == "zip" {
        extract_zip_to_dir(artifact_path, &staging_root)?;
    } else if is_dynamic_library_file(artifact_path) {
        let file_name = artifact_path
            .file_name()
            .ok_or_else(|| anyhow!("invalid artifact path: {}", artifact_path.display()))?;
        let dst = staging_root.join(file_name);
        std::fs::copy(artifact_path, &dst)
            .with_context(|| format!("copy {} -> {}", artifact_path.display(), dst.display()))?;
    } else {
        return Err(anyhow!(
            "unsupported plugin artifact: {} (expect .zip or .{})",
            artifact_path.display(),
            dynamic_library_ext()
        ));
    }

    let mut valid = Vec::<(PathBuf, PluginManifest)>::new();
    for candidate in find_plugin_library_candidates(&staging_root) {
        if let Ok(manifest) = inspect_plugin_library_at(&candidate, &staging_root) {
            valid.push((candidate, manifest))
        }
    }

    let (library_path, manifest) = match valid.len() {
        0 => {
            return Err(anyhow!(
                "no valid StellaTune plugin library found in artifact: {}",
                artifact_path.display()
            ));
        }
        1 => valid.pop().expect("single item"),
        _ => {
            let ids = valid
                .iter()
                .map(|(_, m)| m.id.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!(
                "artifact contains multiple StellaTune plugins ({ids}); one artifact must contain exactly one plugin"
            ));
        }
    };

    let library_rel_path = library_path
        .strip_prefix(&staging_root)
        .with_context(|| {
            format!(
                "failed to derive library_rel_path from {}",
                library_path.display()
            )
        })?
        .to_path_buf();
    let install_root = plugins_dir.join(&manifest.id);
    if install_root.exists() {
        std::fs::remove_dir_all(&install_root)
            .with_context(|| format!("remove {}", install_root.display()))?;
    }
    copy_dir_recursive(&staging_root, &install_root)?;

    let library_rel_path_string = library_rel_path.to_string_lossy().replace('\\', "/");
    let receipt = PluginInstallReceipt {
        manifest: manifest.clone(),
        library_rel_path: library_rel_path_string,
    };
    write_receipt(&install_root, &receipt)?;

    let installed = InstalledPluginInfo {
        id: manifest.id.clone(),
        name: manifest.name.unwrap_or_else(|| manifest.id.clone()),
        root_dir: install_root.clone(),
        library_path: install_root.join(library_rel_path),
        info_json: metadata_info_json(manifest.metadata.as_ref()),
        install_state: PluginInstallState::Installed,
        uninstall_retry_count: 0,
        uninstall_last_error: None,
    };

    info!(
        target: "stellatune_plugins::install",
        plugin_id = %installed.id,
        plugin_name = %installed.name,
        install_root = %installed.root_dir.display(),
        library_path = %installed.library_path.display(),
        installed_at_ms = now_unix_ms(),
        "plugin installed"
    );
    Ok(installed)
}

pub fn list_installed_plugins(plugins_dir: impl AsRef<Path>) -> Result<Vec<InstalledPluginInfo>> {
    let mut out = Vec::new();
    let plugins_dir = plugins_dir.as_ref();
    for d in manifest::discover_plugins(plugins_dir)? {
        out.push(InstalledPluginInfo {
            id: d.manifest.id.clone(),
            name: d
                .manifest
                .name
                .clone()
                .unwrap_or_else(|| d.manifest.id.clone()),
            root_dir: d.root_dir.clone(),
            library_path: d.library_path.clone(),
            info_json: metadata_info_json(d.manifest.metadata.as_ref()),
            install_state: PluginInstallState::Installed,
            uninstall_retry_count: 0,
            uninstall_last_error: None,
        });
    }
    for pending in manifest::discover_pending_uninstalls(plugins_dir)? {
        let receipt = pending.receipt.as_ref();
        let id = receipt
            .map(|r| r.manifest.id.clone())
            .unwrap_or_else(|| pending.marker.plugin_id.clone());
        let name = receipt
            .and_then(|r| r.manifest.name.clone())
            .unwrap_or_else(|| id.clone());
        let library_path = receipt
            .map(|r| pending.root_dir.join(&r.library_rel_path))
            .unwrap_or_else(|| pending.root_dir.clone());
        let info_json = receipt.and_then(|r| metadata_info_json(r.manifest.metadata.as_ref()));
        out.push(InstalledPluginInfo {
            id,
            name,
            root_dir: pending.root_dir.clone(),
            library_path,
            info_json,
            install_state: pending.marker.state,
            uninstall_retry_count: pending.marker.retry_count,
            uninstall_last_error: pending.marker.last_error.clone(),
        });
    }
    out.sort_by(|a, b| {
        a.id.cmp(&b.id).then_with(|| {
            a.root_dir
                .to_string_lossy()
                .cmp(&b.root_dir.to_string_lossy())
        })
    });
    Ok(out)
}

pub fn uninstall_plugin(plugins_dir: impl AsRef<Path>, plugin_id: &str) -> Result<()> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Err(anyhow!("plugin_id is empty"));
    }
    let plugins_dir = plugins_dir.as_ref();
    let mut roots = Vec::new();
    for d in manifest::discover_plugins(plugins_dir)? {
        if d.manifest.id == plugin_id {
            roots.push(d.root_dir);
        }
    }
    for pending in manifest::discover_pending_uninstalls(plugins_dir)? {
        let pending_id = pending
            .receipt
            .as_ref()
            .map(|r| r.manifest.id.as_str())
            .unwrap_or(pending.marker.plugin_id.as_str());
        if pending_id == plugin_id {
            roots.push(pending.root_dir);
        }
    }
    if roots.is_empty() {
        return Err(anyhow!("plugin not installed: {plugin_id}"));
    }
    roots.sort();
    roots.dedup();
    for root in roots {
        if root.exists() {
            match std::fs::remove_dir_all(&root) {
                Ok(()) => {}
                Err(remove_err) => {
                    let marker_path = pending_marker_path_for_plugin_root(&root);
                    let marker = UninstallPendingMarker {
                        plugin_id: plugin_id.to_string(),
                        queued_at_ms: now_unix_ms(),
                        retry_count: 0,
                        last_error: Some(remove_err.to_string()),
                        state: PluginInstallState::PendingUninstall,
                    };
                    write_uninstall_pending_marker(&marker_path, &marker)?;
                    warn!(
                        target: "stellatune_plugins::uninstall",
                        plugin_id = %plugin_id,
                        root = %root.display(),
                        marker = %marker_path.display(),
                        "plugin uninstall deferred; will retry on next discovery"
                    );
                }
            }
        }
    }
    Ok(())
}
