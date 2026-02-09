use core::ffi::c_void;

use crate::{StLogLevel, StStatus, StStr, StVersion};

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
    /// Poll next host event from host to plugin.
    /// On success:
    /// - empty means no event
    /// - non-empty is host-owned and must be freed by `free_host_str_utf8`
    pub poll_host_event_json_utf8:
        Option<extern "C" fn(user_data: *mut c_void, out_event_json_utf8: *mut StStr) -> StStatus>,
    /// Send control request and receive immediate response JSON.
    /// `out_response_json_utf8` is host-allocated and must be released by `free_host_str_utf8`.
    pub send_control_json_utf8: Option<
        extern "C" fn(
            user_data: *mut c_void,
            request_json_utf8: StStr,
            out_response_json_utf8: *mut StStr,
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

    pub create_decoder_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StDecoderInstanceRef,
        ) -> StStatus,
    >,
    pub create_dsp_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            sample_rate: u32,
            channels: u16,
            config_json_utf8: StStr,
            out_instance: *mut StDspInstanceRef,
        ) -> StStatus,
    >,
    pub create_source_catalog_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StSourceCatalogInstanceRef,
        ) -> StStatus,
    >,
    pub create_lyrics_provider_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StLyricsProviderInstanceRef,
        ) -> StStatus,
    >,
    pub create_output_sink_instance: Option<
        extern "C" fn(
            type_id_utf8: StStr,
            config_json_utf8: StStr,
            out_instance: *mut StOutputSinkInstanceRef,
        ) -> StStatus,
    >,

    /// Optional module shutdown hook before host drops the dynamic library generation.
    pub shutdown: Option<extern "C" fn() -> StStatus>,
}
