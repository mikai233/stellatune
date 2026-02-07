mod manifest;
mod util;

use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use libloading::{Library, Symbol};
use stellatune_plugin_api::{
    ST_ERR_INTERNAL, ST_ERR_INVALID_ARG, ST_ERR_IO, ST_INTERFACE_LYRICS_PROVIDER_V1,
    ST_INTERFACE_OUTPUT_SINK_V1, ST_INTERFACE_SOURCE_CATALOG_V1, STELLATUNE_PLUGIN_API_VERSION_V1,
    STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1, StAudioSpec, StDecoderInfoV1, StDecoderOpenArgsV1,
    StDecoderVTableV1, StHostVTableV1, StIoVTableV1, StLogLevel, StLyricsProviderVTableV1,
    StOutputSinkVTableV1, StPluginEntryV1, StPluginVTableV1, StSeekWhence, StSourceCatalogVTableV1,
    StStatus, StStr,
};
use tracing::{debug, info, warn};

pub use manifest::{DiscoveredPlugin, PluginManifest, discover_plugins};

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
    }
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
    if status.code != 0 && status.message.len != 0 {
        if let Some(free) = plugin_free {
            (free)(
                status.message.ptr as *mut core::ffi::c_void,
                status.message.len,
                1,
            );
        }
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

#[derive(Debug)]
pub struct LoadedPlugin {
    pub root_dir: PathBuf,
    pub manifest: PluginManifest,
    pub library_path: PathBuf,
    pub library: PluginLibrary,
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

pub struct PluginManager {
    host: StHostVTableV1,
    plugins: Vec<LoadedPlugin>,
    disabled_ids: HashSet<String>,
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

pub struct SourceStream {
    io_vtable: *const StIoVTableV1,
    io_handle: *mut core::ffi::c_void,
    close_stream: extern "C" fn(io_handle: *mut core::ffi::c_void),
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
        if self.io_handle.is_null() {
            return;
        }
        (self.close_stream)(self.io_handle);
        self.io_handle = core::ptr::null_mut();
    }
}

pub struct OutputSinkInstance {
    handle: *mut core::ffi::c_void,
    vtable: *const StOutputSinkVTableV1,
    plugin_free: Option<extern "C" fn(ptr: *mut core::ffi::c_void, len: usize, align: usize)>,
}

unsafe impl Send for OutputSinkInstance {}

impl OutputSinkInstance {
    pub fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> Result<u32> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("output sink instance is not initialized"));
        }
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
        let Some(flush) = (unsafe { (*self.vtable).flush }) else {
            return Ok(());
        };
        let status = (flush)(self.handle);
        status_to_result("Output sink flush", status, self.plugin_free)
    }
}

impl Drop for OutputSinkInstance {
    fn drop(&mut self) {
        if self.handle.is_null() || self.vtable.is_null() {
            return;
        }
        unsafe { ((*self.vtable).close)(self.handle) };
        self.handle = core::ptr::null_mut();
    }
}

pub struct DspInstance {
    handle: *mut core::ffi::c_void,
    vtable: *const stellatune_plugin_api::StDspVTableV1,
}

unsafe impl Send for DspInstance {}

impl DspInstance {
    pub fn process_in_place(&mut self, samples: &mut [f32], frames: u32) {
        if self.handle.is_null() || self.vtable.is_null() {
            return;
        }
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
        unsafe { ((*self.vtable).supported_layouts)() }
    }

    /// Returns the output channel count if this DSP changes the channel count.
    /// Returns 0 if the DSP preserves the input channel count (passthrough).
    pub fn output_channels(&self) -> u16 {
        if self.vtable.is_null() {
            return 0;
        }
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
        if self.handle.is_null() || self.vtable.is_null() {
            return;
        }
        unsafe { ((*self.vtable).drop)(self.handle) };
        self.handle = core::ptr::null_mut();
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
    _io_guard: DecoderIoGuard,
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
        let mut s = StStr::empty();
        let status = (get)(self.handle, &mut s);
        status_to_result("Decoder get_metadata_json", status, self.plugin_free)?;
        if s.ptr.is_null() || s.len == 0 {
            return Ok(None);
        }
        let text = take_plugin_string(s, self.plugin_free);
        Ok(Some(text))
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<()> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("Decoder instance is not initialized"));
        }
        let Some(seek) = (unsafe { (*self.vtable).seek_ms }) else {
            return Err(anyhow!("Decoder seek not supported"));
        };
        let status = (seek)(self.handle, position_ms);
        status_to_result("Decoder seek", status, self.plugin_free)
    }

    pub fn read_interleaved_f32(&mut self, frames: u32) -> Result<(Vec<f32>, bool)> {
        if self.handle.is_null() || self.vtable.is_null() {
            return Err(anyhow!("Decoder instance is not initialized"));
        }
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
        if self.handle.is_null() || self.vtable.is_null() {
            return;
        }
        unsafe { ((*self.vtable).close)(self.handle) };
        self.handle = core::ptr::null_mut();
    }
}

impl PluginManager {
    pub fn new(host: StHostVTableV1) -> Self {
        Self {
            host,
            plugins: Vec::new(),
            disabled_ids: HashSet::new(),
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

    pub fn list_dsp_types(&self) -> Vec<DspTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library.vtable();
            if pv.is_null() {
                continue;
            }
            let plugin_id = p.library.id();
            let plugin_name = p.library.name();
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
            let pv = p.library.vtable();
            if pv.is_null() {
                continue;
            }
            let plugin_id = p.library.id();
            let plugin_name = p.library.name();
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

    pub fn list_source_catalog_types(&self) -> Vec<SourceCatalogTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let vt = p.library.get_interface_raw(ST_INTERFACE_SOURCE_CATALOG_V1)
                as *const StSourceCatalogVTableV1;
            if vt.is_null() {
                continue;
            }
            let plugin_id = p.library.id();
            let plugin_name = p.library.name();
            let type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
            let display_name = unsafe { util::ststr_to_string_lossy(((*vt).display_name_utf8)()) };
            let config_schema_json =
                unsafe { util::ststr_to_string_lossy(((*vt).config_schema_json_utf8)()) };
            let default_config_json =
                unsafe { util::ststr_to_string_lossy(((*vt).default_config_json_utf8)()) };
            out.push(SourceCatalogTypeInfo {
                key: SourceCatalogKey { plugin_index },
                plugin_id,
                plugin_name,
                type_id,
                display_name,
                config_schema_json,
                default_config_json,
            });
        }
        out
    }

    pub fn list_lyrics_provider_types(&self) -> Vec<LyricsProviderTypeInfo> {
        let mut out = Vec::new();
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let vt = p.library.get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
                as *const StLyricsProviderVTableV1;
            if vt.is_null() {
                continue;
            }
            let plugin_id = p.library.id();
            let plugin_name = p.library.name();
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
            let vt = p.library.get_interface_raw(ST_INTERFACE_OUTPUT_SINK_V1)
                as *const StOutputSinkVTableV1;
            if vt.is_null() {
                continue;
            }
            let plugin_id = p.library.id();
            let plugin_name = p.library.name();
            let type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
            let display_name = unsafe { util::ststr_to_string_lossy(((*vt).display_name_utf8)()) };
            let config_schema_json =
                unsafe { util::ststr_to_string_lossy(((*vt).config_schema_json_utf8)()) };
            let default_config_json =
                unsafe { util::ststr_to_string_lossy(((*vt).default_config_json_utf8)()) };
            out.push(OutputSinkTypeInfo {
                key: OutputSinkKey { plugin_index },
                plugin_id,
                plugin_name,
                type_id,
                display_name,
                config_schema_json,
                default_config_json,
            });
        }
        out
    }

    pub fn find_source_catalog_key(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Option<SourceCatalogKey> {
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if p.library.id() != plugin_id || self.is_disabled(&p.manifest.id) {
                continue;
            }
            let vt = p.library.get_interface_raw(ST_INTERFACE_SOURCE_CATALOG_V1)
                as *const StSourceCatalogVTableV1;
            if vt.is_null() {
                continue;
            }
            let tid = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
            if tid == type_id {
                return Some(SourceCatalogKey { plugin_index });
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
            if p.library.id() != plugin_id || self.is_disabled(&p.manifest.id) {
                continue;
            }
            let vt = p.library.get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
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
            if p.library.id() != plugin_id || self.is_disabled(&p.manifest.id) {
                continue;
            }
            let vt = p.library.get_interface_raw(ST_INTERFACE_OUTPUT_SINK_V1)
                as *const StOutputSinkVTableV1;
            if vt.is_null() {
                continue;
            }
            let tid = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
            if tid == type_id {
                return Some(OutputSinkKey { plugin_index });
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
        let vt = p.library.get_interface_raw(ST_INTERFACE_SOURCE_CATALOG_V1)
            as *const StSourceCatalogVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide source catalog interface",
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
        let vt = p.library.get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
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
        let vt =
            p.library.get_interface_raw(ST_INTERFACE_OUTPUT_SINK_V1) as *const StOutputSinkVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide output sink interface",
                p.manifest.id
            ));
        }
        Ok(vt)
    }

    pub fn source_list_items_json(
        &self,
        key: SourceCatalogKey,
        config_json: &str,
        request_json: &str,
    ) -> Result<String> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt = p.library.get_interface_raw(ST_INTERFACE_SOURCE_CATALOG_V1)
            as *const StSourceCatalogVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide source catalog interface",
                p.manifest.id
            ));
        }
        let plugin_free = p.library.plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let req = StStr {
            ptr: request_json.as_ptr(),
            len: request_json.len(),
        };
        let mut out = StStr::empty();
        let status = unsafe { ((*vt).list_items_json_utf8)(cfg, req, &mut out) };
        status_to_result("Source list_items_json", status, plugin_free)?;
        Ok(take_plugin_string(out, plugin_free))
    }

    pub fn source_open_stream(
        &self,
        key: SourceCatalogKey,
        config_json: &str,
        track_json: &str,
    ) -> Result<(SourceStream, Option<String>)> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt = p.library.get_interface_raw(ST_INTERFACE_SOURCE_CATALOG_V1)
            as *const StSourceCatalogVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide source catalog interface",
                p.manifest.id
            ));
        }
        let plugin_free = p.library.plugin_free();
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
        let meta = take_plugin_string(out_track_meta_json_utf8, plugin_free);
        Ok((
            SourceStream {
                io_vtable: out_io_vtable,
                io_handle: out_io_handle,
                close_stream: unsafe { (*vt).close_stream },
            },
            if meta.is_empty() { None } else { Some(meta) },
        ))
    }

    pub fn lyrics_search_json(&self, key: LyricsProviderKey, query_json: &str) -> Result<String> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt = p.library.get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
            as *const StLyricsProviderVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide lyrics provider interface",
                p.manifest.id
            ));
        }
        let plugin_free = p.library.plugin_free();
        let query = StStr {
            ptr: query_json.as_ptr(),
            len: query_json.len(),
        };
        let mut out = StStr::empty();
        let status = unsafe { ((*vt).search_json_utf8)(query, &mut out) };
        status_to_result("Lyrics search_json", status, plugin_free)?;
        Ok(take_plugin_string(out, plugin_free))
    }

    pub fn lyrics_fetch_json(&self, key: LyricsProviderKey, track_json: &str) -> Result<String> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt = p.library.get_interface_raw(ST_INTERFACE_LYRICS_PROVIDER_V1)
            as *const StLyricsProviderVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide lyrics provider interface",
                p.manifest.id
            ));
        }
        let plugin_free = p.library.plugin_free();
        let track = StStr {
            ptr: track_json.as_ptr(),
            len: track_json.len(),
        };
        let mut out = StStr::empty();
        let status = unsafe { ((*vt).fetch_json_utf8)(track, &mut out) };
        status_to_result("Lyrics fetch_json", status, plugin_free)?;
        Ok(take_plugin_string(out, plugin_free))
    }

    pub fn output_list_targets_json(
        &self,
        key: OutputSinkKey,
        config_json: &str,
    ) -> Result<String> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt =
            p.library.get_interface_raw(ST_INTERFACE_OUTPUT_SINK_V1) as *const StOutputSinkVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide output sink interface",
                p.manifest.id
            ));
        }
        let plugin_free = p.library.plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let mut out = StStr::empty();
        let status = unsafe { ((*vt).list_targets_json_utf8)(cfg, &mut out) };
        status_to_result("Output sink list_targets_json", status, plugin_free)?;
        Ok(take_plugin_string(out, plugin_free))
    }

    pub fn output_open(
        &self,
        key: OutputSinkKey,
        config_json: &str,
        target_json: &str,
        spec: StAudioSpec,
    ) -> Result<OutputSinkInstance> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let vt =
            p.library.get_interface_raw(ST_INTERFACE_OUTPUT_SINK_V1) as *const StOutputSinkVTableV1;
        if vt.is_null() {
            return Err(anyhow!(
                "plugin `{}` does not provide output sink interface",
                p.manifest.id
            ));
        }
        let plugin_free = p.library.plugin_free();
        let cfg = StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let target = StStr {
            ptr: target_json.as_ptr(),
            len: target_json.len(),
        };
        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let status = unsafe { ((*vt).open)(cfg, target, spec, &mut handle) };
        status_to_result("Output sink open", status, plugin_free)?;
        if handle.is_null() {
            return Err(anyhow!("output sink open returned null handle"));
        }
        Ok(OutputSinkInstance {
            handle,
            vtable: vt,
            plugin_free,
        })
    }

    pub fn find_dsp_key(&self, plugin_id: &str, type_id: &str) -> Option<DspKey> {
        for (plugin_index, p) in self.plugins.iter().enumerate() {
            if p.library.id() != plugin_id {
                continue;
            }
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library.vtable();
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
            if p.library.id() != plugin_id {
                continue;
            }
            if self.is_disabled(&p.manifest.id) {
                continue;
            }
            let pv = p.library.vtable();
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
        let pv = p.library.vtable();
        if pv.is_null() {
            return Err(anyhow!("plugin has null vtable"));
        }
        let vt = unsafe { ((*pv).decoder_get)(key.decoder_index) };
        if vt.is_null() {
            return Err(anyhow!("invalid decoder_index {}", key.decoder_index));
        }
        let decoder_type_id = unsafe { util::ststr_to_string_lossy(((*vt).type_id_utf8)()) };
        let plugin_id = p.library.id();
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

        let plugin_free = p.library.plugin_free();
        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
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
        let status = unsafe { ((*vt).get_info)(handle, &mut info) };
        if status.code != 0 {
            unsafe { ((*vt).close)(handle) };
            return Err(status_err_to_anyhow(
                "Decoder get_info",
                status,
                plugin_free,
            ));
        }

        Ok(DecoderInstance {
            handle,
            vtable: vt,
            info,
            plugin_free,
            _io_guard: io_guard,
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
            let pv = p.library.vtable();
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
                plugin_id = %p.library.id(),
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
            let pv = p.library.vtable();
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
            let pv = p.library.vtable();
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
                plugin_id = %p.library.id(),
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
            let pv = p.library.vtable();
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

    pub fn create_dsp(
        &self,
        key: DspKey,
        sample_rate: u32,
        channels: u16,
        config_json: &str,
    ) -> Result<DspInstance> {
        let p = self
            .plugins
            .get(key.plugin_index)
            .ok_or_else(|| anyhow!("invalid plugin_index {}", key.plugin_index))?;
        if self.is_disabled(&p.manifest.id) {
            return Err(anyhow!("plugin `{}` is disabled", p.manifest.id));
        }
        let pv = p.library.vtable();
        if pv.is_null() {
            return Err(anyhow!("plugin has null vtable"));
        }
        let vt = unsafe { ((*pv).dsp_get)(key.dsp_index) };
        if vt.is_null() {
            return Err(anyhow!("invalid dsp_index {}", key.dsp_index));
        }

        let mut handle: *mut core::ffi::c_void = core::ptr::null_mut();
        let json = stellatune_plugin_api::StStr {
            ptr: config_json.as_ptr(),
            len: config_json.len(),
        };
        let status = unsafe { ((*vt).create)(sample_rate, channels, json, &mut handle) };
        if status.code != 0 || handle.is_null() {
            return Err(anyhow!("DSP create failed (code={})", status.code));
        }

        Ok(DspInstance { handle, vtable: vt })
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
                        plugin_id = %loaded.library.id(),
                        plugin_name = %loaded.library.name(),
                        root_dir = %loaded.root_dir.display(),
                        library_path = %loaded.library_path.display(),
                        decoders = loaded.library.decoder_count(),
                        dsps = loaded.library.dsp_count(),
                        "plugin loaded"
                    );
                    report.loaded.push(LoadedPluginInfo {
                        id: loaded.library.id(),
                        name: loaded.library.name(),
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
                        plugin_id = %loaded.library.id(),
                        plugin_name = %loaded.library.name(),
                        root_dir = %loaded.root_dir.display(),
                        library_path = %loaded.library_path.display(),
                        decoders = loaded.library.decoder_count(),
                        dsps = loaded.library.dsp_count(),
                        "plugin loaded"
                    );
                    report.loaded.push(LoadedPluginInfo {
                        id: loaded.library.id(),
                        name: loaded.library.name(),
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

        let rel = discovered.manifest.library_path_for_current_platform()?;
        let library_path = discovered.root_dir.join(rel);
        if !library_path.exists() {
            return Err(anyhow!(
                "plugin `{}` library not found: {}",
                discovered.manifest.id,
                library_path.display()
            ));
        }

        let entry_symbol = discovered
            .manifest
            .entry_symbol
            .as_deref()
            .unwrap_or(STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1);

        let library = unsafe { PluginLibrary::load(&library_path, entry_symbol, &self.host)? };

        let exported_id = library.id();
        if exported_id != discovered.manifest.id {
            return Err(anyhow!(
                "plugin id mismatch: manifest.id=`{}`, exported.id=`{}`",
                discovered.manifest.id,
                exported_id
            ));
        }

        Ok(LoadedPlugin {
            root_dir: discovered.root_dir.clone(),
            manifest: discovered.manifest.clone(),
            library_path,
            library,
        })
    }
}
