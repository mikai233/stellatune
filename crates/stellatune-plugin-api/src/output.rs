use core::ffi::c_void;

use crate::{StAudioSpec, StStatus, StStr};

use super::StConfigUpdatePlan;

pub type StOutputSinkNegotiatedSpec = crate::StOutputSinkNegotiatedSpec;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StOutputSinkInstanceRef {
    pub handle: *mut c_void,
    pub vtable: *const StOutputSinkInstanceVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StOutputSinkInstanceVTable {
    pub list_targets_json_utf8:
        extern "C" fn(handle: *mut c_void, out_json_utf8: *mut StStr) -> StStatus,
    pub negotiate_spec: extern "C" fn(
        handle: *mut c_void,
        target_json_utf8: StStr,
        desired_spec: StAudioSpec,
        out_negotiated: *mut StOutputSinkNegotiatedSpec,
    ) -> StStatus,
    pub open:
        extern "C" fn(handle: *mut c_void, target_json_utf8: StStr, spec: StAudioSpec) -> StStatus,
    pub write_interleaved_f32: extern "C" fn(
        handle: *mut c_void,
        frames: u32,
        channels: u16,
        samples: *const f32,
        out_frames_accepted: *mut u32,
    ) -> StStatus,
    pub flush: Option<extern "C" fn(handle: *mut c_void) -> StStatus>,
    pub close: extern "C" fn(handle: *mut c_void),

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
