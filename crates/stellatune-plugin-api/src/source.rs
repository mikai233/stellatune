use core::ffi::c_void;

use crate::{
    StAsyncOpState, StConfigUpdatePlanOpRef, StIoVTable, StJsonOpRef, StOpNotifier, StStatus,
    StStr, StUnitOpRef,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StSourceListItemsOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StSourceListItemsOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StSourceListItemsOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_json_utf8: extern "C" fn(handle: *mut c_void, out_json_utf8: *mut StStr) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StSourceOpenStreamOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StSourceOpenStreamOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StSourceOpenStreamOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_stream: extern "C" fn(
        handle: *mut c_void,
        out_io_vtable: *mut *const StIoVTable,
        out_io_handle: *mut *mut c_void,
        out_track_meta_json_utf8: *mut StStr,
    ) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StSourceCatalogInstanceRef {
    pub handle: *mut c_void,
    pub vtable: *const StSourceCatalogInstanceVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StSourceCatalogInstanceVTable {
    pub begin_list_items_json_utf8: extern "C" fn(
        handle: *mut c_void,
        request_json_utf8: StStr,
        out_op: *mut StSourceListItemsOpRef,
    ) -> StStatus,
    pub begin_open_stream: extern "C" fn(
        handle: *mut c_void,
        track_json_utf8: StStr,
        out_op: *mut StSourceOpenStreamOpRef,
    ) -> StStatus,
    pub begin_close_stream: extern "C" fn(
        handle: *mut c_void,
        io_handle: *mut c_void,
        out_op: *mut StUnitOpRef,
    ) -> StStatus,

    pub begin_plan_config_update_json_utf8: Option<
        extern "C" fn(
            handle: *mut c_void,
            new_config_json_utf8: StStr,
            out_op: *mut StConfigUpdatePlanOpRef,
        ) -> StStatus,
    >,
    pub begin_apply_config_update_json_utf8: Option<
        extern "C" fn(
            handle: *mut c_void,
            new_config_json_utf8: StStr,
            out_op: *mut StUnitOpRef,
        ) -> StStatus,
    >,
    pub begin_export_state_json_utf8:
        Option<extern "C" fn(handle: *mut c_void, out_op: *mut StJsonOpRef) -> StStatus>,
    pub begin_import_state_json_utf8: Option<
        extern "C" fn(
            handle: *mut c_void,
            state_json_utf8: StStr,
            out_op: *mut StUnitOpRef,
        ) -> StStatus,
    >,

    pub destroy: extern "C" fn(handle: *mut c_void),
}
