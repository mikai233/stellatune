use core::ffi::c_void;

use crate::{StStatus, StStr};

use super::StConfigUpdatePlanV2;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StDspInstanceRefV2 {
    pub handle: *mut c_void,
    pub vtable: *const StDspInstanceVTableV2,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StDspInstanceVTableV2 {
    pub process_interleaved_f32_in_place:
        extern "C" fn(handle: *mut c_void, samples: *mut f32, frames: u32),
    /// Returns bitmask of supported input channel layouts (ST_LAYOUT_* flags).
    pub supported_layouts: extern "C" fn(handle: *mut c_void) -> u32,
    /// Returns output channel count if this DSP changes channel count. 0 means passthrough.
    pub output_channels: extern "C" fn(handle: *mut c_void) -> u16,

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
