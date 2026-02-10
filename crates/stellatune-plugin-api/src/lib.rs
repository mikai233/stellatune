#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).
#![allow(clippy::missing_safety_doc)]

use core::ffi::c_void;

mod common;
mod decoder;
mod dsp;
mod lyrics;
mod module;
mod output;
mod source;

pub use common::{StCapabilityKind, StConfigUpdateMode, StConfigUpdatePlan};
pub use decoder::*;
pub use dsp::*;
pub use lyrics::*;
pub use module::*;
pub use output::*;
pub use source::*;

// Single in-development ABI version (early-stage project).
// Note: this ABI may change in place during early development.
pub const STELLATUNE_PLUGIN_API_VERSION: u32 = 5;
pub const STELLATUNE_PLUGIN_ENTRY_SYMBOL: &str = "stellatune_plugin_entry";

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

// Immutable byte view used across FFI boundaries. Callers are responsible for lifetime validity.
unsafe impl Send for StStr {}
unsafe impl Sync for StStr {}

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
pub struct StIoVTable {
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

pub const ST_DECODER_INFO_FLAG_SEEKABLE: u32 = 1 << 0;
pub const ST_DECODER_INFO_FLAG_HAS_DURATION: u32 = 1 << 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDecoderInfo {
    pub spec: StAudioSpec,
    /// Only valid when `flags & ST_DECODER_INFO_FLAG_HAS_DURATION != 0`.
    pub duration_ms: u64,
    pub flags: u32,
    pub reserved: u32,
}

pub const ST_OUTPUT_NEGOTIATE_EXACT: u32 = 1 << 0;
pub const ST_OUTPUT_NEGOTIATE_CHANGED_SR: u32 = 1 << 1;
pub const ST_OUTPUT_NEGOTIATE_CHANGED_CH: u32 = 1 << 2;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StOutputSinkNegotiatedSpec {
    pub spec: StAudioSpec,
    /// Plugin preferred write chunk in frames. 0 means "no preference".
    pub preferred_chunk_frames: u32,
    pub flags: u32,
    pub reserved: u32,
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
