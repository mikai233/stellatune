use core::ffi::c_void;

use crate::{
    StAsyncOpState, StJsonOpRef, StLogLevel, StOpNotifier, StStatus, StStr, StUnitOpRef, StVersion,
};

use super::{
    StDecoderExtScore, StDecoderInstanceRef, StDspInstanceRef, StLyricsProviderInstanceRef,
    StOutputSinkInstanceRef, StSourceCatalogInstanceRef,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StCapabilityDescriptor {
    pub kind: super::StCapabilityKind,
    pub type_id_utf8: StStr,
    pub display_name_utf8: StStr,
    pub config_schema_json_utf8: StStr,
    pub default_config_json_utf8: StStr,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StHostVTable {
    pub api_version: u32,
    pub user_data: *mut c_void,
    pub log_utf8: Option<extern "C" fn(user_data: *mut c_void, level: StLogLevel, msg: StStr)>,
    /// Returns host runtime root path as UTF-8 bytes.
    /// The returned bytes are host-owned and read-only.
    pub get_runtime_root_utf8: Option<extern "C" fn(user_data: *mut c_void) -> StStr>,
    /// Emit runtime event from plugin to host.
    pub emit_event_json_utf8:
        Option<extern "C" fn(user_data: *mut c_void, event_json_utf8: StStr) -> StStatus>,
    /// Begin polling next host event from host to plugin.
    /// Result JSON is host-owned and must be freed by `free_host_str_utf8`.
    pub begin_poll_host_event_json_utf8:
        Option<extern "C" fn(user_data: *mut c_void, out_op: *mut StJsonOpRef) -> StStatus>,
    /// Begin sending control request and receiving response JSON.
    /// Result JSON is host-allocated and must be released by `free_host_str_utf8`.
    pub begin_send_control_json_utf8: Option<
        extern "C" fn(
            user_data: *mut c_void,
            request_json_utf8: StStr,
            out_op: *mut StJsonOpRef,
        ) -> StStatus,
    >,
    /// Free host-owned UTF-8 strings returned by callbacks above.
    pub free_host_str_utf8: Option<extern "C" fn(user_data: *mut c_void, s: StStr)>,
}

// Raw pointers make this not auto-Send/Sync. Host vtable is treated as immutable and requires
// `user_data` to be thread-safe when used across threads.
unsafe impl Send for StHostVTable {}
unsafe impl Sync for StHostVTable {}

pub type StPluginEntry = unsafe extern "C" fn(host: *const StHostVTable) -> *const StPluginModule;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StCreateDecoderInstanceOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StCreateDecoderInstanceOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StCreateDecoderInstanceOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_instance:
        extern "C" fn(handle: *mut c_void, out_instance: *mut StDecoderInstanceRef) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StCreateDspInstanceOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StCreateDspInstanceOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StCreateDspInstanceOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_instance:
        extern "C" fn(handle: *mut c_void, out_instance: *mut StDspInstanceRef) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StCreateSourceCatalogInstanceOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StCreateSourceCatalogInstanceOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StCreateSourceCatalogInstanceOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_instance: extern "C" fn(
        handle: *mut c_void,
        out_instance: *mut StSourceCatalogInstanceRef,
    ) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StCreateLyricsProviderInstanceOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StCreateLyricsProviderInstanceOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StCreateLyricsProviderInstanceOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_instance: extern "C" fn(
        handle: *mut c_void,
        out_instance: *mut StLyricsProviderInstanceRef,
    ) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StCreateOutputSinkInstanceOpRef {
    pub handle: *mut c_void,
    pub vtable: *const StCreateOutputSinkInstanceOpVTable,
    pub reserved0: u32,
    pub reserved1: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StCreateOutputSinkInstanceOpVTable {
    pub poll: extern "C" fn(handle: *mut c_void, out_state: *mut StAsyncOpState) -> StStatus,
    pub wait: extern "C" fn(
        handle: *mut c_void,
        timeout_ms: u32,
        out_state: *mut StAsyncOpState,
    ) -> StStatus,
    pub cancel: extern "C" fn(handle: *mut c_void) -> StStatus,
    pub set_notifier: extern "C" fn(handle: *mut c_void, notifier: StOpNotifier) -> StStatus,
    pub take_instance:
        extern "C" fn(handle: *mut c_void, out_instance: *mut StOutputSinkInstanceRef) -> StStatus,
    pub destroy: extern "C" fn(handle: *mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StPluginModule {
    pub api_version: u32,
    pub plugin_version: StVersion,
    /// Optional free hook for plugin-owned UTF-8 bytes returned by plugin APIs.
    pub plugin_free: Option<extern "C" fn(ptr: *mut c_void, len: usize, align: usize)>,
    pub metadata_json_utf8: extern "C" fn() -> StStr,

    pub capability_count: extern "C" fn() -> usize,
    pub capability_get: extern "C" fn(index: usize) -> *const StCapabilityDescriptor,

    /// Optional decoder extension scoring table access.
    /// Host may use this to rank decoder candidates by extension without content probing.
    pub decoder_ext_score_count: Option<extern "C" fn(type_id_utf8: StStr) -> usize>,
    pub decoder_ext_score_get:
        Option<extern "C" fn(type_id_utf8: StStr, index: usize) -> *const StDecoderExtScore>,

    pub begin_create_decoder_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_op: *mut StCreateDecoderInstanceOpRef,
        ) -> StStatus,
    >,
    pub begin_create_dsp_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            sample_rate: u32,
            channels: u16,
            config_json_utf8: StStr,
            out_op: *mut StCreateDspInstanceOpRef,
        ) -> StStatus,
    >,
    pub begin_create_source_catalog_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_op: *mut StCreateSourceCatalogInstanceOpRef,
        ) -> StStatus,
    >,
    pub begin_create_lyrics_provider_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_op: *mut StCreateLyricsProviderInstanceOpRef,
        ) -> StStatus,
    >,
    pub begin_create_output_sink_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_op: *mut StCreateOutputSinkInstanceOpRef,
        ) -> StStatus,
    >,

    /// Freeze module and reject new begin_* calls for deterministic drain/unload.
    pub begin_quiesce: Option<extern "C" fn(out_op: *mut StUnitOpRef) -> StStatus>,
    /// Optional module shutdown hook before host drops the dynamic library generation.
    pub begin_shutdown: Option<extern "C" fn(out_op: *mut StUnitOpRef) -> StStatus>,
}
