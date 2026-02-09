use core::ffi::c_void;

use crate::{StIoVTableV1, StStatus, StStr};

use super::StConfigUpdatePlanV2;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StSourceCatalogInstanceRefV2 {
    pub handle: *mut c_void,
    pub vtable: *const StSourceCatalogInstanceVTableV2,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StSourceCatalogInstanceVTableV2 {
    pub list_items_json_utf8: extern "C" fn(
        handle: *mut c_void,
        request_json_utf8: StStr,
        out_json_utf8: *mut StStr,
    ) -> StStatus,
    pub open_stream: extern "C" fn(
        handle: *mut c_void,
        track_json_utf8: StStr,
        out_io_vtable: *mut *const StIoVTableV1,
        out_io_handle: *mut *mut c_void,
        out_track_meta_json_utf8: *mut StStr,
    ) -> StStatus,
    pub close_stream: extern "C" fn(handle: *mut c_void, io_handle: *mut c_void),

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
