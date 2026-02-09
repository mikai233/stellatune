use core::ffi::c_void;

use crate::{StDecoderInfo, StIoVTable, StStatus, StStr};

use super::StConfigUpdatePlan;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDecoderExtScore {
    /// Lowercase extension without dot (e.g. "flac").
    /// "*" means wildcard fallback rule.
    pub ext_utf8: StStr,
    /// Higher score wins for decoder candidate ordering.
    pub score: u16,
    /// Reserved for future decoder rule flags.
    pub flags: u16,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDecoderInstanceRef {
    pub handle: *mut c_void,
    pub vtable: *const StDecoderInstanceVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDecoderOpenArgs {
    /// Optional path hint for diagnostics/logging.
    pub path_utf8: StStr,
    /// Optional extension hint (lowercase, no leading dot recommended).
    pub ext_utf8: StStr,
    /// Host-owned IO callback table.
    pub io_vtable: *const StIoVTable,
    /// Host-owned IO handle.
    pub io_handle: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StDecoderInstanceVTable {
    pub open: extern "C" fn(handle: *mut c_void, args: StDecoderOpenArgs) -> StStatus,
    pub get_info: extern "C" fn(handle: *mut c_void, out_info: *mut StDecoderInfo) -> StStatus,
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

    pub plan_config_update_json_utf8: Option<
        extern "C" fn(
            handle: *mut c_void,
            new_config_json_utf8: StStr,
            out_plan: *mut StConfigUpdatePlan,
        ) -> StStatus,
    >,
    pub apply_config_update_json_utf8:
        Option<extern "C" fn(handle: *mut c_void, new_config_json_utf8: StStr) -> StStatus>,
    pub export_state_json_utf8:
        Option<extern "C" fn(handle: *mut c_void, out_json_utf8: *mut StStr) -> StStatus>,
    pub import_state_json_utf8:
        Option<extern "C" fn(handle: *mut c_void, state_json_utf8: StStr) -> StStatus>,

    pub destroy: extern "C" fn(handle: *mut c_void),
}
