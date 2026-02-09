use core::ffi::c_void;

use crate::{StStatus, StStr};

use super::StConfigUpdatePlan;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StLyricsProviderInstanceRef {
    pub handle: *mut c_void,
    pub vtable: *const StLyricsProviderInstanceVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StLyricsProviderInstanceVTable {
    pub search_json_utf8: extern "C" fn(
        handle: *mut c_void,
        query_json_utf8: StStr,
        out_json_utf8: *mut StStr,
    ) -> StStatus,
    pub fetch_json_utf8: extern "C" fn(
        handle: *mut c_void,
        track_json_utf8: StStr,
        out_json_utf8: *mut StStr,
    ) -> StStatus,

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
