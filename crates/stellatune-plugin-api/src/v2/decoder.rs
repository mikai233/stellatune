use core::ffi::c_void;

use crate::{StDecoderInfoV1, StIoVTableV1, StStatus, StStr};

use super::StConfigUpdatePlanV2;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDecoderExtScoreV2 {
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
pub struct StDecoderInstanceRefV2 {
    pub handle: *mut c_void,
    pub vtable: *const StDecoderInstanceVTableV2,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDecoderOpenArgsV2 {
    /// Optional path hint for diagnostics/logging.
    pub path_utf8: StStr,
    /// Optional extension hint (lowercase, no leading dot recommended).
    pub ext_utf8: StStr,
    /// Host-owned IO callback table.
    pub io_vtable: *const StIoVTableV1,
    /// Host-owned IO handle.
    pub io_handle: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StDecoderInstanceVTableV2 {
    pub open: extern "C" fn(handle: *mut c_void, args: StDecoderOpenArgsV2) -> StStatus,
    pub get_info: extern "C" fn(handle: *mut c_void, out_info: *mut StDecoderInfoV1) -> StStatus,
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
            out_plan: *mut StConfigUpdatePlanV2,
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
