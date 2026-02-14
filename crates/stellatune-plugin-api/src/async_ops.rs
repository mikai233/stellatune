use core::ffi::c_void;

use crate::{StConfigUpdatePlan, StStatus, StStr};

pub type StOpNotifyFn = extern "C" fn(user_data: *mut c_void);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StOpNotifier {
    pub user_data: *mut c_void,
    pub notify: Option<StOpNotifyFn>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StAsyncOpState {
    Pending = 0,
    Ready = 1,
    Cancelled = 2,
    Failed = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StUnitOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StUnitOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StUnitOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub finish: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StJsonOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StJsonOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StJsonOpVTable {
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
pub struct StConfigUpdatePlanOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StConfigUpdatePlanOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StConfigUpdatePlanOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_plan:
        extern "C" fn(handle: *mut c_void, out_plan: *mut StConfigUpdatePlan) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}
