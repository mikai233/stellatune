#![allow(clippy::missing_safety_doc)]

use core::ffi::c_void;

// Single in-development ABI version (early-stage project).
// Note: changing ABI without changing this can break older native plugins.
// ABI was changed by adding `get_interface` to `StPluginVTableV1`, `metadata_json_utf8`,
// and `get_runtime_root_utf8` host callback.
// Bump host/plugin API version to reject stale binaries at load time.
pub const STELLATUNE_PLUGIN_API_VERSION_V1: u32 = 3;
pub const STELLATUNE_PLUGIN_ENTRY_SYMBOL_V1: &str = "stellatune_plugin_entry_v1";
pub const ST_INTERFACE_SOURCE_CATALOG_V1: &str = "stellatune.source_catalog.v1";
pub const ST_INTERFACE_LYRICS_PROVIDER_V1: &str = "stellatune.lyrics_provider.v1";
pub const ST_INTERFACE_OUTPUT_SINK_V1: &str = "stellatune.output_sink.v1";

// Status codes (non-exhaustive). Plugins may use other non-zero codes, but the SDK uses these.
pub const ST_ERR_INVALID_ARG: i32 = 1;
pub const ST_ERR_UNSUPPORTED: i32 = 2;
pub const ST_ERR_IO: i32 = 3;
pub const ST_ERR_DECODE: i32 = 4;
pub const ST_ERR_INTERNAL: i32 = 5;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub reserved: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StLogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

/// Immutable UTF-8 bytes. Not NUL-terminated.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StStr {
    pub ptr: *const u8,
    pub len: usize,
}

impl StStr {
    pub const fn empty() -> Self {
        Self {
            ptr: core::ptr::null(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StSlice<T> {
    pub ptr: *const T,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StStatus {
    /// 0 = OK, non-zero = error.
    pub code: i32,
    /// Optional error message (plugin-owned; free via `plugin_free`).
    pub message: StStr,
}

impl StStatus {
    pub const fn ok() -> Self {
        Self {
            code: 0,
            message: StStr::empty(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StAudioSpec {
    pub sample_rate: u32,
    pub channels: u16,
    pub reserved: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StSeekWhence {
    Start = 0,
    Current = 1,
    End = 2,
}

/// Host-provided IO callbacks for streaming decode.
///
/// Ownership: The IO `handle` is owned by the host and must remain valid until the decoder is
/// closed. The decoder must not free/close it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StIoVTableV1 {
    pub read: extern "C" fn(
        handle: *mut c_void,
        out: *mut u8,
        len: usize,
        out_read: *mut usize,
    ) -> StStatus,
    pub seek: Option<
        extern "C" fn(
            handle: *mut c_void,
            offset: i64,
            whence: StSeekWhence,
            out_pos: *mut u64,
        ) -> StStatus,
    >,
    pub tell: Option<extern "C" fn(handle: *mut c_void, out_pos: *mut u64) -> StStatus>,
    pub size: Option<extern "C" fn(handle: *mut c_void, out_size: *mut u64) -> StStatus>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StDecoderOpenArgsV1 {
    /// Optional path (for diagnostics only). May be empty.
    pub path_utf8: StStr,
    /// Optional extension/content-hint (for diagnostics only). May be empty.
    pub ext_utf8: StStr,
    pub io_vtable: *const StIoVTableV1,
    pub io_handle: *mut c_void,
}

pub const ST_DECODER_INFO_FLAG_SEEKABLE: u32 = 1 << 0;
pub const ST_DECODER_INFO_FLAG_HAS_DURATION: u32 = 1 << 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDecoderInfoV1 {
    pub spec: StAudioSpec,
    /// Only valid when `flags & ST_DECODER_INFO_FLAG_HAS_DURATION != 0`.
    pub duration_ms: u64,
    pub flags: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StHostVTableV1 {
    pub api_version: u32,
    pub user_data: *mut c_void,
    pub log_utf8: Option<extern "C" fn(user_data: *mut c_void, level: StLogLevel, msg: StStr)>,
    /// Returns plugin runtime root path as UTF-8 bytes.
    /// The returned bytes are host-owned and must be treated as read-only.
    pub get_runtime_root_utf8: Option<extern "C" fn(user_data: *mut c_void) -> StStr>,
}

// Raw pointers make this not auto-Send/Sync. For StellaTune v1 we treat the host vtable as
// immutable and require any `user_data` it points to to be thread-safe if used.
unsafe impl Send for StHostVTableV1 {}
unsafe impl Sync for StHostVTableV1 {}

pub type StPluginEntryV1 =
    unsafe extern "C" fn(host: *const StHostVTableV1) -> *const StPluginVTableV1;
pub type StPluginGetInterfaceV1 = extern "C" fn(interface_id_utf8: StStr) -> *const c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StPluginVTableV1 {
    pub api_version: u32,
    pub plugin_version: StVersion,
    pub plugin_free: Option<extern "C" fn(ptr: *mut c_void, len: usize, align: usize)>,

    pub id_utf8: extern "C" fn() -> StStr,
    pub name_utf8: extern "C" fn() -> StStr,
    /// Plugin metadata JSON for host-side installation and diagnostics.
    pub metadata_json_utf8: extern "C" fn() -> StStr,

    pub decoder_count: extern "C" fn() -> usize,
    pub decoder_get: extern "C" fn(index: usize) -> *const StDecoderVTableV1,

    pub dsp_count: extern "C" fn() -> usize,
    pub dsp_get: extern "C" fn(index: usize) -> *const StDspVTableV1,
    /// Optional interface lookup for non-decoder/DSP plugin capabilities.
    ///
    /// Pass one of `ST_INTERFACE_*_V1` and cast the returned pointer accordingly.
    /// Returns null when unsupported.
    pub get_interface: Option<StPluginGetInterfaceV1>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StDecoderVTableV1 {
    pub type_id_utf8: extern "C" fn() -> StStr,
    /// Return a score [0..100]. Higher wins. 0 means "not supported".
    pub probe: extern "C" fn(path_ext_utf8: StStr, header: StSlice<u8>) -> u8,

    pub open: extern "C" fn(args: StDecoderOpenArgsV1, out: *mut *mut c_void) -> StStatus,
    pub get_info: extern "C" fn(handle: *mut c_void, out_info: *mut StDecoderInfoV1) -> StStatus,
    /// Optional JSON metadata (tags + codec/container info).
    pub get_metadata_json_utf8:
        Option<extern "C" fn(handle: *mut c_void, out_json: *mut StStr) -> StStatus>,

    pub read_interleaved_f32: extern "C" fn(
        handle: *mut c_void,
        frames: u32,
        out_interleaved: *mut f32,
        out_frames_read: *mut u32,
        out_eof: *mut bool,
    ) -> StStatus,
    pub seek_ms: Option<extern "C" fn(handle: *mut c_void, position_ms: u64) -> StStatus>,
    pub close: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StDspVTableV1 {
    pub type_id_utf8: extern "C" fn() -> StStr,
    pub display_name_utf8: extern "C" fn() -> StStr,
    pub config_schema_json_utf8: extern "C" fn() -> StStr,
    pub default_config_json_utf8: extern "C" fn() -> StStr,

    pub create: extern "C" fn(
        sample_rate: u32,
        channels: u16,
        config_json_utf8: StStr,
        out: *mut *mut c_void,
    ) -> StStatus,
    pub set_config_json_utf8:
        extern "C" fn(handle: *mut c_void, config_json_utf8: StStr) -> StStatus,
    pub process_interleaved_f32_in_place:
        extern "C" fn(handle: *mut c_void, samples: *mut f32, frames: u32),
    pub drop: extern "C" fn(handle: *mut c_void),

    /// Returns bitmask of supported input channel layouts (ST_LAYOUT_* flags).
    /// If this returns 0 or ST_LAYOUT_STEREO, the DSP only supports stereo.
    pub supported_layouts: extern "C" fn() -> u32,

    /// Returns the output channel count if this DSP changes the channel count.
    /// Returns 0 if the DSP preserves the input channel count (passthrough).
    pub output_channels: extern "C" fn() -> u16,
}

/// Optional source-catalog interface.
///
/// JSON contracts are plugin-defined. Host passes/receives UTF-8 JSON blobs.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StSourceCatalogVTableV1 {
    pub type_id_utf8: extern "C" fn() -> StStr,
    pub display_name_utf8: extern "C" fn() -> StStr,
    pub config_schema_json_utf8: extern "C" fn() -> StStr,
    pub default_config_json_utf8: extern "C" fn() -> StStr,
    pub list_items_json_utf8: extern "C" fn(
        config_json_utf8: StStr,
        request_json_utf8: StStr,
        out_json_utf8: *mut StStr,
    ) -> StStatus,
    pub open_stream: extern "C" fn(
        config_json_utf8: StStr,
        track_json_utf8: StStr,
        out_io_vtable: *mut *const StIoVTableV1,
        out_io_handle: *mut *mut c_void,
        out_track_meta_json_utf8: *mut StStr,
    ) -> StStatus,
    pub close_stream: extern "C" fn(io_handle: *mut c_void),
}

/// Optional lyrics-provider interface.
///
/// JSON contracts are plugin-defined. Host passes/receives UTF-8 JSON blobs.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StLyricsProviderVTableV1 {
    pub type_id_utf8: extern "C" fn() -> StStr,
    pub display_name_utf8: extern "C" fn() -> StStr,
    pub search_json_utf8:
        extern "C" fn(query_json_utf8: StStr, out_json_utf8: *mut StStr) -> StStatus,
    pub fetch_json_utf8:
        extern "C" fn(track_json_utf8: StStr, out_json_utf8: *mut StStr) -> StStatus,
}

/// Optional output-sink interface.
///
/// JSON contracts are plugin-defined. Host passes/receives UTF-8 JSON blobs.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StOutputSinkVTableV1 {
    pub type_id_utf8: extern "C" fn() -> StStr,
    pub display_name_utf8: extern "C" fn() -> StStr,
    pub config_schema_json_utf8: extern "C" fn() -> StStr,
    pub default_config_json_utf8: extern "C" fn() -> StStr,
    pub list_targets_json_utf8:
        extern "C" fn(config_json_utf8: StStr, out_json_utf8: *mut StStr) -> StStatus,
    pub open: extern "C" fn(
        config_json_utf8: StStr,
        target_json_utf8: StStr,
        spec: StAudioSpec,
        out_handle: *mut *mut c_void,
    ) -> StStatus,
    pub write_interleaved_f32: extern "C" fn(
        handle: *mut c_void,
        frames: u32,
        channels: u16,
        samples: *const f32,
        out_frames_accepted: *mut u32,
    ) -> StStatus,
    pub flush: Option<extern "C" fn(handle: *mut c_void) -> StStatus>,
    pub close: extern "C" fn(handle: *mut c_void),
}

// Channel layout bitmask flags for DSP plugins.
/// Mono (1 channel)
pub const ST_LAYOUT_MONO: u32 = 1 << 0;
/// Stereo (2 channels)
pub const ST_LAYOUT_STEREO: u32 = 1 << 1;
/// 5.1 Surround (6 channels)
pub const ST_LAYOUT_5_1: u32 = 1 << 2;
/// 7.1 Surround (8 channels)
pub const ST_LAYOUT_7_1: u32 = 1 << 3;
/// Supports any channel layout
pub const ST_LAYOUT_ANY: u32 = 0xFFFFFFFF;
