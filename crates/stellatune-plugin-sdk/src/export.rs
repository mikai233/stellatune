use core::sync::atomic::{AtomicPtr, Ordering};

use stellatune_plugin_api::{
    STELLATUNE_PLUGIN_API_VERSION, STELLATUNE_PLUGIN_ENTRY_SYMBOL, StCapabilityDescriptor,
    StCapabilityKind, StHostVTable, StPluginModule,
};

use crate::{StStr, ststr};

static HOST_VTABLE: AtomicPtr<StHostVTable> = AtomicPtr::new(core::ptr::null_mut());

#[doc(hidden)]
pub unsafe fn __set_host_vtable(host: *const StHostVTable) {
    HOST_VTABLE.store(host as *mut StHostVTable, Ordering::Release);
}

pub fn host_vtable_raw() -> Option<*const StHostVTable> {
    let p = HOST_VTABLE.load(Ordering::Acquire);
    if p.is_null() {
        None
    } else {
        Some(p as *const _)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityDescriptorStatic {
    pub kind: StCapabilityKind,
    pub type_id_utf8: &'static str,
    pub display_name_utf8: &'static str,
    pub config_schema_json_utf8: &'static str,
    pub default_config_json_utf8: &'static str,
}

impl CapabilityDescriptorStatic {
    pub const fn to_ffi(self) -> StCapabilityDescriptor {
        StCapabilityDescriptor {
            kind: self.kind,
            type_id_utf8: StStr {
                ptr: self.type_id_utf8.as_ptr(),
                len: self.type_id_utf8.len(),
            },
            display_name_utf8: StStr {
                ptr: self.display_name_utf8.as_ptr(),
                len: self.display_name_utf8.len(),
            },
            config_schema_json_utf8: StStr {
                ptr: self.config_schema_json_utf8.as_ptr(),
                len: self.config_schema_json_utf8.len(),
            },
            default_config_json_utf8: StStr {
                ptr: self.default_config_json_utf8.as_ptr(),
                len: self.default_config_json_utf8.len(),
            },
            reserved0: 0,
            reserved1: 0,
        }
    }
}

pub fn static_str_ststr(s: &'static str) -> StStr {
    ststr(s)
}

pub fn entry_symbol() -> &'static str {
    STELLATUNE_PLUGIN_ENTRY_SYMBOL
}

#[doc(hidden)]
#[macro_export]
macro_rules! __st_opt_create_cb {
    () => {
        None
    };
    ($f:path) => {
        Some($f)
    };
}

/// Minimal module exporter.
///
/// This macro intentionally provides a narrow bootstrap surface so migration can proceed
/// incrementally. Capability-specific export macros can be layered on top later.
#[macro_export]
macro_rules! export_plugin_minimal {
    (
        metadata_json_utf8: $metadata_json_utf8:path,
        capability_count: $capability_count:path,
        capability_get: $capability_get:path
        $(, create_decoder_instance: $create_decoder_instance:path)?
        $(, create_dsp_instance: $create_dsp_instance:path)?
        $(, create_source_catalog_instance: $create_source_catalog_instance:path)?
        $(, create_lyrics_provider_instance: $create_lyrics_provider_instance:path)?
        $(, create_output_sink_instance: $create_output_sink_instance:path)?
        $(, shutdown: $shutdown:path)?
        $(,)?
    ) => {
        static __ST_PLUGIN_MODULE: stellatune_plugin_api::StPluginModule =
            stellatune_plugin_api::StPluginModule {
                api_version: stellatune_plugin_api::STELLATUNE_PLUGIN_API_VERSION,
                plugin_version: stellatune_plugin_api::StVersion {
                    major: 0,
                    minor: 1,
                    patch: 0,
                    reserved: 0,
                },
                plugin_free: Some($crate::plugin_free),
                metadata_json_utf8: $metadata_json_utf8,
                capability_count: $capability_count,
                capability_get: $capability_get,
                decoder_ext_score_count: None,
                decoder_ext_score_get: None,
                create_decoder_instance: $crate::__st_opt_create_cb!($($create_decoder_instance)?),
                create_dsp_instance: $crate::__st_opt_create_cb!($($create_dsp_instance)?),
                create_source_catalog_instance: $crate::__st_opt_create_cb!($($create_source_catalog_instance)?),
                create_lyrics_provider_instance: $crate::__st_opt_create_cb!($($create_lyrics_provider_instance)?),
                create_output_sink_instance: $crate::__st_opt_create_cb!($($create_output_sink_instance)?),
                shutdown: $crate::__st_opt_create_cb!($($shutdown)?),
            };

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn stellatune_plugin_entry(
            host: *const stellatune_plugin_api::StHostVTable,
        ) -> *const stellatune_plugin_api::StPluginModule {
            unsafe { $crate::export::__set_host_vtable(host) };
            &__ST_PLUGIN_MODULE
        }
    };
}

pub fn module_api_version() -> u32 {
    STELLATUNE_PLUGIN_API_VERSION
}

pub fn module_is_current(module: &StPluginModule) -> bool {
    module.api_version == STELLATUNE_PLUGIN_API_VERSION
}

#[macro_export]
macro_rules! export_plugin {
    (
        id: $plugin_id:literal,
        name: $plugin_name:literal,
        version: ($vmaj:literal, $vmin:literal, $vpatch:literal),
        decoders: [
            $($dec_mod:ident => $dec_ty:ty),* $(,)?
        ],
        dsps: [
            $($dsp_mod:ident => $dsp_ty:ty),* $(,)?
        ],
        source_catalogs: [
            $($source_mod:ident => $source_ty:ty),* $(,)?
        ],
        lyrics_providers: [
            $($lyrics_mod:ident => $lyrics_ty:ty),* $(,)?
        ],
        output_sinks: [
            $($sink_mod:ident => $sink_ty:ty),* $(,)?
        ]
        $(, info: $info:expr)?
        $(,)?
    ) => {
        const __ST_PLUGIN_ID: &str = $plugin_id;
        const __ST_PLUGIN_NAME: &str = $plugin_name;

        fn __st_plugin_metadata_json() -> &'static str {
            static META: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            META.get_or_init(|| {
                let metadata = $crate::build_plugin_metadata_with_info(
                    __ST_PLUGIN_ID,
                    __ST_PLUGIN_NAME,
                    $vmaj,
                    $vmin,
                    $vpatch,
                    $crate::__st_opt_info!($($info)?),
                );
                $crate::__private::serde_json::to_string(&metadata)
                    .expect("export_plugin! metadata must be serializable")
            })
        }

        extern "C" fn __st_plugin_metadata_json_utf8() -> $crate::StStr {
            let s = __st_plugin_metadata_json();
            $crate::StStr {
                ptr: s.as_ptr(),
                len: s.len(),
            }
        }

        $(
            mod $dec_mod {
                use super::*;

                pub static CAP_DESC: stellatune_plugin_api::StCapabilityDescriptor =
                    stellatune_plugin_api::StCapabilityDescriptor {
                        kind: stellatune_plugin_api::StCapabilityKind::Decoder,
                        type_id_utf8: $crate::ststr(<$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID),
                        display_name_utf8: $crate::ststr(<$dec_ty as $crate::instance::DecoderDescriptor>::DISPLAY_NAME),
                        config_schema_json_utf8: $crate::ststr(<$dec_ty as $crate::instance::DecoderDescriptor>::CONFIG_SCHEMA_JSON),
                        default_config_json_utf8: $crate::ststr(<$dec_ty as $crate::instance::DecoderDescriptor>::DEFAULT_CONFIG_JSON),
                        reserved0: 0,
                        reserved1: 0,
                    };

                fn ext_score_rules_ffi() -> &'static [stellatune_plugin_api::StDecoderExtScore] {
                    static RULES: std::sync::OnceLock<Vec<stellatune_plugin_api::StDecoderExtScore>> =
                        std::sync::OnceLock::new();
                    RULES
                        .get_or_init(|| {
                            <$dec_ty as $crate::instance::DecoderDescriptor>::EXT_SCORE_RULES
                                .iter()
                                .map(|rule| stellatune_plugin_api::StDecoderExtScore {
                                    ext_utf8: $crate::ststr(rule.ext),
                                    score: rule.score,
                                    flags: 0,
                                    reserved: 0,
                                })
                                .collect()
                        })
                        .as_slice()
                }

                pub extern "C" fn ext_score_count() -> usize {
                    ext_score_rules_ffi().len()
                }

                pub extern "C" fn ext_score_get(
                    index: usize,
                ) -> *const stellatune_plugin_api::StDecoderExtScore {
                    ext_score_rules_ffi()
                        .get(index)
                        .map(|v| v as *const _)
                        .unwrap_or(core::ptr::null())
                }

                extern "C" fn open(
                    handle: *mut core::ffi::c_void,
                    args: stellatune_plugin_api::StDecoderOpenArgs,
                ) -> $crate::StStatus {
                    if handle.is_null() || args.io_vtable.is_null() || args.io_handle.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle/io_vtable/io_handle",
                        );
                    }
                    let path_hint = match unsafe { $crate::ststr_to_str(&args.path_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let ext_hint = match unsafe { $crate::ststr_to_str(&args.ext_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    let io = $crate::instance::DecoderOpenIoRef {
                        io_vtable: args.io_vtable,
                        io_handle: args.io_handle,
                    };
                    let open_args = $crate::instance::DecoderOpenArgsRef {
                        path_hint,
                        ext_hint,
                        io,
                    };
                    match <$dec_ty as $crate::instance::DecoderInstance>::open(&mut boxed.inner, open_args) {
                        Ok(()) => {
                            let info = <$dec_ty as $crate::instance::DecoderInstance>::get_info(&boxed.inner);
                            boxed.channels = info.spec.channels.max(1);
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, e),
                    }
                }

                extern "C" fn get_info(
                    handle: *mut core::ffi::c_void,
                    out_info: *mut $crate::StDecoderInfo,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_info.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_info");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    let info = <$dec_ty as $crate::instance::DecoderInstance>::get_info(&boxed.inner);
                    boxed.channels = info.spec.channels.max(1);
                    unsafe { *out_info = info; }
                    $crate::status_ok()
                }

                extern "C" fn get_metadata_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::instance::DecoderInstance>::get_metadata_json(&boxed.inner) {
                        Ok(Some(json)) => {
                            unsafe { *out_json = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Ok(None) => {
                            unsafe { *out_json = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn read_interleaved_f32(
                    handle: *mut core::ffi::c_void,
                    frames: u32,
                    out_interleaved: *mut f32,
                    out_frames_read: *mut u32,
                    out_eof: *mut bool,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_interleaved.is_null() || out_frames_read.is_null() || out_eof.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null handle/out_interleaved/out_frames_read/out_eof",
                        );
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    let len = (frames as usize).saturating_mul(boxed.channels as usize);
                    let out = unsafe { core::slice::from_raw_parts_mut(out_interleaved, len) };
                    match <$dec_ty as $crate::instance::DecoderInstance>::read_interleaved_f32(
                        &mut boxed.inner,
                        frames,
                        out,
                    ) {
                        Ok((n, eof)) => {
                            unsafe {
                                *out_frames_read = n;
                                *out_eof = eof;
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_DECODE, e),
                    }
                }

                extern "C" fn seek_ms(
                    handle: *mut core::ffi::c_void,
                    position_ms: u64,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::instance::DecoderInstance>::seek_ms(&mut boxed.inner, position_ms) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, e),
                    }
                }

                extern "C" fn plan_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_plan: *mut stellatune_plugin_api::StConfigUpdatePlan,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_plan.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_plan");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::update::ConfigUpdatable>::plan_config_update_json(&boxed.inner, new_json) {
                        Ok(plan) => match $crate::update::write_plan_to_ffi(out_plan, plan) {
                            Ok(()) => $crate::status_ok(),
                            Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                        },
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn apply_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::update::ConfigUpdatable>::apply_config_update_json(&mut boxed.inner, new_json) {
                        Ok(()) => {
                            let info = <$dec_ty as $crate::instance::DecoderInstance>::get_info(&boxed.inner);
                            boxed.channels = info.spec.channels.max(1);
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn export_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::update::ConfigUpdatable>::export_state_json(&boxed.inner) {
                        Ok(Some(json)) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Ok(None) => {
                            unsafe { *out_json_utf8 = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn import_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    state_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let state_json = match unsafe { $crate::ststr_to_str(&state_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                    match <$dec_ty as $crate::update::ConfigUpdatable>::import_state_json(&mut boxed.inner, state_json) {
                        Ok(()) => {
                            let info = <$dec_ty as $crate::instance::DecoderInstance>::get_info(&boxed.inner);
                            boxed.channels = info.spec.channels.max(1);
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn destroy(handle: *mut core::ffi::c_void) {
                    if handle.is_null() {
                        return;
                    }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::instance::DecoderBox<$dec_ty>)); }
                }

                pub static VTABLE: stellatune_plugin_api::StDecoderInstanceVTable =
                    stellatune_plugin_api::StDecoderInstanceVTable {
                        open,
                        get_info,
                        get_metadata_json_utf8: Some(get_metadata_json_utf8),
                        read_interleaved_f32,
                        seek_ms: Some(seek_ms),
                        plan_config_update_json_utf8: Some(plan_config_update_json_utf8),
                        apply_config_update_json_utf8: Some(apply_config_update_json_utf8),
                        export_state_json_utf8: Some(export_state_json_utf8),
                        import_state_json_utf8: Some(import_state_json_utf8),
                        destroy,
                    };

                pub extern "C" fn create_instance(
                    config_json_utf8: $crate::StStr,
                    out_instance: *mut stellatune_plugin_api::StDecoderInstanceRef,
                ) -> $crate::StStatus {
                    if out_instance.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_instance");
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$dec_ty as $crate::instance::DecoderDescriptor>::Config>(json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e.to_string()),
                    };
                    match <$dec_ty as $crate::instance::DecoderDescriptor>::create(config) {
                        Ok(instance) => {
                            let channels = instance.get_info().spec.channels.max(1);
                            let boxed = Box::new($crate::instance::DecoderBox {
                                inner: instance,
                                channels,
                            });
                            unsafe {
                                *out_instance = stellatune_plugin_api::StDecoderInstanceRef {
                                    handle: Box::into_raw(boxed) as *mut core::ffi::c_void,
                                    vtable: &VTABLE as *const _,
                                    reserved0: 0,
                                    reserved1: 0,
                                };
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_DECODE, e),
                    }
                }
            }
        )*

        $(
            mod $dsp_mod {
                use super::*;

                pub static CAP_DESC: stellatune_plugin_api::StCapabilityDescriptor =
                    stellatune_plugin_api::StCapabilityDescriptor {
                        kind: stellatune_plugin_api::StCapabilityKind::Dsp,
                        type_id_utf8: $crate::ststr(<$dsp_ty as $crate::instance::DspDescriptor>::TYPE_ID),
                        display_name_utf8: $crate::ststr(<$dsp_ty as $crate::instance::DspDescriptor>::DISPLAY_NAME),
                        config_schema_json_utf8: $crate::ststr(<$dsp_ty as $crate::instance::DspDescriptor>::CONFIG_SCHEMA_JSON),
                        default_config_json_utf8: $crate::ststr(<$dsp_ty as $crate::instance::DspDescriptor>::DEFAULT_CONFIG_JSON),
                        reserved0: 0,
                        reserved1: 0,
                    };

                extern "C" fn process_interleaved_f32_in_place(
                    handle: *mut core::ffi::c_void,
                    samples: *mut f32,
                    frames: u32,
                ) {
                    if handle.is_null() || samples.is_null() {
                        return;
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DspBox<$dsp_ty>) };
                    let len = (frames as usize).saturating_mul(boxed.channels as usize);
                    let buf = unsafe { core::slice::from_raw_parts_mut(samples, len) };
                    <$dsp_ty as $crate::instance::DspInstance>::process_interleaved_f32_in_place(
                        &mut boxed.inner,
                        buf,
                        frames,
                    );
                }

                extern "C" fn supported_layouts(handle: *mut core::ffi::c_void) -> u32 {
                    if handle.is_null() {
                        return $crate::ST_LAYOUT_STEREO;
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DspBox<$dsp_ty>) };
                    <$dsp_ty as $crate::instance::DspInstance>::supported_layouts(&boxed.inner)
                }

                extern "C" fn output_channels(handle: *mut core::ffi::c_void) -> u16 {
                    if handle.is_null() {
                        return 0;
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DspBox<$dsp_ty>) };
                    <$dsp_ty as $crate::instance::DspInstance>::output_channels(&boxed.inner)
                }

                extern "C" fn plan_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_plan: *mut stellatune_plugin_api::StConfigUpdatePlan,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_plan.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_plan");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DspBox<$dsp_ty>) };
                    match <$dsp_ty as $crate::update::ConfigUpdatable>::plan_config_update_json(&boxed.inner, new_json) {
                        Ok(plan) => match $crate::update::write_plan_to_ffi(out_plan, plan) {
                            Ok(()) => $crate::status_ok(),
                            Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                        },
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn apply_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DspBox<$dsp_ty>) };
                    match <$dsp_ty as $crate::update::ConfigUpdatable>::apply_config_update_json(&mut boxed.inner, new_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn export_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DspBox<$dsp_ty>) };
                    match <$dsp_ty as $crate::update::ConfigUpdatable>::export_state_json(&boxed.inner) {
                        Ok(Some(json)) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Ok(None) => {
                            unsafe { *out_json_utf8 = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn import_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    state_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let state_json = match unsafe { $crate::ststr_to_str(&state_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::DspBox<$dsp_ty>) };
                    match <$dsp_ty as $crate::update::ConfigUpdatable>::import_state_json(&mut boxed.inner, state_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn destroy(handle: *mut core::ffi::c_void) {
                    if handle.is_null() {
                        return;
                    }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::instance::DspBox<$dsp_ty>)); }
                }

                pub static VTABLE: stellatune_plugin_api::StDspInstanceVTable =
                    stellatune_plugin_api::StDspInstanceVTable {
                        process_interleaved_f32_in_place,
                        supported_layouts,
                        output_channels,
                        plan_config_update_json_utf8: Some(plan_config_update_json_utf8),
                        apply_config_update_json_utf8: Some(apply_config_update_json_utf8),
                        export_state_json_utf8: Some(export_state_json_utf8),
                        import_state_json_utf8: Some(import_state_json_utf8),
                        destroy,
                    };

                pub extern "C" fn create_instance(
                    sample_rate: u32,
                    channels: u16,
                    config_json_utf8: $crate::StStr,
                    out_instance: *mut stellatune_plugin_api::StDspInstanceRef,
                ) -> $crate::StStatus {
                    if out_instance.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_instance");
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$dsp_ty as $crate::instance::DspDescriptor>::Config>(json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e.to_string()),
                    };
                    let spec = $crate::StAudioSpec {
                        sample_rate: sample_rate.max(1),
                        channels: channels.max(1),
                        reserved: 0,
                    };
                    match <$dsp_ty as $crate::instance::DspDescriptor>::create(spec, config) {
                        Ok(instance) => {
                            let boxed = Box::new($crate::instance::DspBox {
                                inner: instance,
                                channels: spec.channels.max(1),
                            });
                            unsafe {
                                *out_instance = stellatune_plugin_api::StDspInstanceRef {
                                    handle: Box::into_raw(boxed) as *mut core::ffi::c_void,
                                    vtable: &VTABLE as *const _,
                                    reserved0: 0,
                                    reserved1: 0,
                                };
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }
            }
        )*

        $(
            mod $source_mod {
                use super::*;

                type CatalogImpl = <$source_ty as $crate::instance::SourceCatalogDescriptor>::Instance;
                type StreamImpl = <CatalogImpl as $crate::instance::SourceCatalogInstance>::Stream;

                pub static CAP_DESC: stellatune_plugin_api::StCapabilityDescriptor =
                    stellatune_plugin_api::StCapabilityDescriptor {
                        kind: stellatune_plugin_api::StCapabilityKind::SourceCatalog,
                        type_id_utf8: $crate::ststr(<$source_ty as $crate::instance::SourceCatalogDescriptor>::TYPE_ID),
                        display_name_utf8: $crate::ststr(<$source_ty as $crate::instance::SourceCatalogDescriptor>::DISPLAY_NAME),
                        config_schema_json_utf8: $crate::ststr(<$source_ty as $crate::instance::SourceCatalogDescriptor>::CONFIG_SCHEMA_JSON),
                        default_config_json_utf8: $crate::ststr(<$source_ty as $crate::instance::SourceCatalogDescriptor>::DEFAULT_CONFIG_JSON),
                        reserved0: 0,
                        reserved1: 0,
                    };

                extern "C" fn list_items_json_utf8(
                    handle: *mut core::ffi::c_void,
                    request_json_utf8: $crate::StStr,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let request_json = match unsafe { $crate::ststr_to_str(&request_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                    match <CatalogImpl as $crate::instance::SourceCatalogInstance>::list_items_json(&mut boxed.inner, request_json) {
                        Ok(json) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn io_read(
                    handle: *mut core::ffi::c_void,
                    out: *mut u8,
                    len: usize,
                    out_read: *mut usize,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_read.is_null() || (len > 0 && out.is_null()) {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "invalid io_read args");
                    }
                    let out_slice: &mut [u8] = if len == 0 {
                        &mut []
                    } else {
                        unsafe { core::slice::from_raw_parts_mut(out, len) }
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::instance::SourceStream>::read(&mut boxed.inner, out_slice) {
                        Ok(n) => {
                            unsafe {
                                *out_read = n.min(len);
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, e),
                    }
                }

                extern "C" fn io_seek(
                    handle: *mut core::ffi::c_void,
                    offset: i64,
                    whence: $crate::StSeekWhence,
                    out_pos: *mut u64,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_pos.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "invalid io_seek args");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::instance::SourceStream>::seek(&mut boxed.inner, offset, whence) {
                        Ok(pos) => {
                            unsafe { *out_pos = pos; }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, e),
                    }
                }

                extern "C" fn io_tell(
                    handle: *mut core::ffi::c_void,
                    out_pos: *mut u64,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_pos.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "invalid io_tell args");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::instance::SourceStream>::tell(&mut boxed.inner) {
                        Ok(pos) => {
                            unsafe { *out_pos = pos; }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, e),
                    }
                }

                extern "C" fn io_size(
                    handle: *mut core::ffi::c_void,
                    out_size: *mut u64,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_size.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "invalid io_size args");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::instance::SourceStream>::size(&mut boxed.inner) {
                        Ok(size) => {
                            unsafe { *out_size = size; }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, e),
                    }
                }

                extern "C" fn open_stream(
                    handle: *mut core::ffi::c_void,
                    track_json_utf8: $crate::StStr,
                    out_io_vtable: *mut *const $crate::StIoVTable,
                    out_io_handle: *mut *mut core::ffi::c_void,
                    out_track_meta_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_io_vtable.is_null() || out_io_handle.is_null() || out_track_meta_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_io_vtable/out_io_handle/out_track_meta_json_utf8");
                    }
                    let track_json = match unsafe { $crate::ststr_to_str(&track_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                    match <CatalogImpl as $crate::instance::SourceCatalogInstance>::open_stream_json(&mut boxed.inner, track_json) {
                        Ok(opened) => {
                            let $crate::instance::SourceOpenResult { stream, track_meta_json } = opened;
                            let stream_boxed = Box::new($crate::instance::SourceStreamBox { inner: stream });
                            unsafe {
                                *out_io_vtable = &IO_VTABLE as *const $crate::StIoVTable;
                                *out_io_handle = Box::into_raw(stream_boxed) as *mut core::ffi::c_void;
                                *out_track_meta_json_utf8 = track_meta_json.as_deref().map($crate::alloc_utf8_bytes).unwrap_or_else($crate::StStr::empty);
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn close_stream(handle: *mut core::ffi::c_void, io_handle: *mut core::ffi::c_void) {
                    if handle.is_null() || io_handle.is_null() {
                        return;
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                    let mut stream = unsafe { Box::from_raw(io_handle as *mut $crate::instance::SourceStreamBox<StreamImpl>) };
                    let _ = <CatalogImpl as $crate::instance::SourceCatalogInstance>::close_stream(&mut boxed.inner, &mut stream.inner);
                }

                extern "C" fn plan_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_plan: *mut stellatune_plugin_api::StConfigUpdatePlan,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_plan.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_plan");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                    match <CatalogImpl as $crate::update::ConfigUpdatable>::plan_config_update_json(&boxed.inner, new_json) {
                        Ok(plan) => match $crate::update::write_plan_to_ffi(out_plan, plan) {
                            Ok(()) => $crate::status_ok(),
                            Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                        },
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn apply_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                    match <CatalogImpl as $crate::update::ConfigUpdatable>::apply_config_update_json(&mut boxed.inner, new_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn export_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                    match <CatalogImpl as $crate::update::ConfigUpdatable>::export_state_json(&boxed.inner) {
                        Ok(Some(json)) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Ok(None) => {
                            unsafe { *out_json_utf8 = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn import_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    state_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let state_json = match unsafe { $crate::ststr_to_str(&state_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                    match <CatalogImpl as $crate::update::ConfigUpdatable>::import_state_json(&mut boxed.inner, state_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn destroy(handle: *mut core::ffi::c_void) {
                    if handle.is_null() {
                        return;
                    }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>)); }
                }

                pub static IO_VTABLE: $crate::StIoVTable = $crate::StIoVTable {
                    read: io_read,
                    seek: if <StreamImpl as $crate::instance::SourceStream>::SUPPORTS_SEEK { Some(io_seek) } else { None },
                    tell: if <StreamImpl as $crate::instance::SourceStream>::SUPPORTS_TELL { Some(io_tell) } else { None },
                    size: if <StreamImpl as $crate::instance::SourceStream>::SUPPORTS_SIZE { Some(io_size) } else { None },
                };

                pub static VTABLE: stellatune_plugin_api::StSourceCatalogInstanceVTable =
                    stellatune_plugin_api::StSourceCatalogInstanceVTable {
                        list_items_json_utf8,
                        open_stream,
                        close_stream,
                        plan_config_update_json_utf8: Some(plan_config_update_json_utf8),
                        apply_config_update_json_utf8: Some(apply_config_update_json_utf8),
                        export_state_json_utf8: Some(export_state_json_utf8),
                        import_state_json_utf8: Some(import_state_json_utf8),
                        destroy,
                    };

                pub extern "C" fn create_instance(
                    config_json_utf8: $crate::StStr,
                    out_instance: *mut stellatune_plugin_api::StSourceCatalogInstanceRef,
                ) -> $crate::StStatus {
                    if out_instance.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_instance");
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$source_ty as $crate::instance::SourceCatalogDescriptor>::Config>(json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e.to_string()),
                    };
                    match <$source_ty as $crate::instance::SourceCatalogDescriptor>::create(config) {
                        Ok(instance) => {
                            let boxed = Box::new($crate::instance::SourceCatalogBox { inner: instance });
                            unsafe {
                                *out_instance = stellatune_plugin_api::StSourceCatalogInstanceRef {
                                    handle: Box::into_raw(boxed) as *mut core::ffi::c_void,
                                    vtable: &VTABLE as *const _,
                                    reserved0: 0,
                                    reserved1: 0,
                                };
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }
            }
        )*

        $(
            mod $lyrics_mod {
                use super::*;

                type LyricsImpl = <$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::Instance;

                pub static CAP_DESC: stellatune_plugin_api::StCapabilityDescriptor =
                    stellatune_plugin_api::StCapabilityDescriptor {
                        kind: stellatune_plugin_api::StCapabilityKind::LyricsProvider,
                        type_id_utf8: $crate::ststr(<$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::TYPE_ID),
                        display_name_utf8: $crate::ststr(<$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::DISPLAY_NAME),
                        config_schema_json_utf8: $crate::ststr(<$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::CONFIG_SCHEMA_JSON),
                        default_config_json_utf8: $crate::ststr(<$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::DEFAULT_CONFIG_JSON),
                        reserved0: 0,
                        reserved1: 0,
                    };

                extern "C" fn search_json_utf8(
                    handle: *mut core::ffi::c_void,
                    query_json_utf8: $crate::StStr,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let query_json = match unsafe { $crate::ststr_to_str(&query_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::LyricsProviderBox<LyricsImpl>) };
                    match <LyricsImpl as $crate::instance::LyricsProviderInstance>::search_json(&mut boxed.inner, query_json) {
                        Ok(json) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn fetch_json_utf8(
                    handle: *mut core::ffi::c_void,
                    track_json_utf8: $crate::StStr,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let track_json = match unsafe { $crate::ststr_to_str(&track_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::LyricsProviderBox<LyricsImpl>) };
                    match <LyricsImpl as $crate::instance::LyricsProviderInstance>::fetch_json(&mut boxed.inner, track_json) {
                        Ok(json) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn plan_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_plan: *mut stellatune_plugin_api::StConfigUpdatePlan,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_plan.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_plan");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::LyricsProviderBox<LyricsImpl>) };
                    match <LyricsImpl as $crate::update::ConfigUpdatable>::plan_config_update_json(&boxed.inner, new_json) {
                        Ok(plan) => match $crate::update::write_plan_to_ffi(out_plan, plan) {
                            Ok(()) => $crate::status_ok(),
                            Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                        },
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn apply_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::LyricsProviderBox<LyricsImpl>) };
                    match <LyricsImpl as $crate::update::ConfigUpdatable>::apply_config_update_json(&mut boxed.inner, new_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn export_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::LyricsProviderBox<LyricsImpl>) };
                    match <LyricsImpl as $crate::update::ConfigUpdatable>::export_state_json(&boxed.inner) {
                        Ok(Some(json)) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Ok(None) => {
                            unsafe { *out_json_utf8 = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn import_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    state_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let state_json = match unsafe { $crate::ststr_to_str(&state_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::LyricsProviderBox<LyricsImpl>) };
                    match <LyricsImpl as $crate::update::ConfigUpdatable>::import_state_json(&mut boxed.inner, state_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn destroy(handle: *mut core::ffi::c_void) {
                    if handle.is_null() {
                        return;
                    }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::instance::LyricsProviderBox<LyricsImpl>)); }
                }

                pub static VTABLE: stellatune_plugin_api::StLyricsProviderInstanceVTable =
                    stellatune_plugin_api::StLyricsProviderInstanceVTable {
                        search_json_utf8,
                        fetch_json_utf8,
                        plan_config_update_json_utf8: Some(plan_config_update_json_utf8),
                        apply_config_update_json_utf8: Some(apply_config_update_json_utf8),
                        export_state_json_utf8: Some(export_state_json_utf8),
                        import_state_json_utf8: Some(import_state_json_utf8),
                        destroy,
                    };

                pub extern "C" fn create_instance(
                    config_json_utf8: $crate::StStr,
                    out_instance: *mut stellatune_plugin_api::StLyricsProviderInstanceRef,
                ) -> $crate::StStatus {
                    if out_instance.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_instance");
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::Config>(json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e.to_string()),
                    };
                    match <$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::create(config) {
                        Ok(instance) => {
                            let boxed = Box::new($crate::instance::LyricsProviderBox { inner: instance });
                            unsafe {
                                *out_instance = stellatune_plugin_api::StLyricsProviderInstanceRef {
                                    handle: Box::into_raw(boxed) as *mut core::ffi::c_void,
                                    vtable: &VTABLE as *const _,
                                    reserved0: 0,
                                    reserved1: 0,
                                };
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }
            }
        )*

        $(
            mod $sink_mod {
                use super::*;

                type SinkImpl = <$sink_ty as $crate::instance::OutputSinkDescriptor>::Instance;

                pub static CAP_DESC: stellatune_plugin_api::StCapabilityDescriptor =
                    stellatune_plugin_api::StCapabilityDescriptor {
                        kind: stellatune_plugin_api::StCapabilityKind::OutputSink,
                        type_id_utf8: $crate::ststr(<$sink_ty as $crate::instance::OutputSinkDescriptor>::TYPE_ID),
                        display_name_utf8: $crate::ststr(<$sink_ty as $crate::instance::OutputSinkDescriptor>::DISPLAY_NAME),
                        config_schema_json_utf8: $crate::ststr(<$sink_ty as $crate::instance::OutputSinkDescriptor>::CONFIG_SCHEMA_JSON),
                        default_config_json_utf8: $crate::ststr(<$sink_ty as $crate::instance::OutputSinkDescriptor>::DEFAULT_CONFIG_JSON),
                        reserved0: 0,
                        reserved1: 0,
                    };

                extern "C" fn list_targets_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::instance::OutputSinkInstance>::list_targets_json(&mut boxed.inner) {
                        Ok(json) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn negotiate_spec(
                    handle: *mut core::ffi::c_void,
                    target_json_utf8: $crate::StStr,
                    desired_spec: $crate::StAudioSpec,
                    out_negotiated: *mut stellatune_plugin_api::StOutputSinkNegotiatedSpec,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_negotiated.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_negotiated");
                    }
                    let target_json = match unsafe { $crate::ststr_to_str(&target_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let desired = $crate::StAudioSpec {
                        sample_rate: desired_spec.sample_rate.max(1),
                        channels: desired_spec.channels.max(1),
                        reserved: 0,
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::instance::OutputSinkInstance>::negotiate_spec_json(&mut boxed.inner, target_json, desired) {
                        Ok(mut negotiated) => {
                            negotiated.spec.sample_rate = negotiated.spec.sample_rate.max(1);
                            negotiated.spec.channels = negotiated.spec.channels.max(1);
                            negotiated.spec.reserved = 0;
                            unsafe { *out_negotiated = negotiated; }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn open(
                    handle: *mut core::ffi::c_void,
                    target_json_utf8: $crate::StStr,
                    spec: $crate::StAudioSpec,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let target_json = match unsafe { $crate::ststr_to_str(&target_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let open_spec = $crate::StAudioSpec {
                        sample_rate: spec.sample_rate.max(1),
                        channels: spec.channels.max(1),
                        reserved: 0,
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::instance::OutputSinkInstance>::open_json(&mut boxed.inner, target_json, open_spec) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn write_interleaved_f32(
                    handle: *mut core::ffi::c_void,
                    frames: u32,
                    channels: u16,
                    samples: *const f32,
                    out_frames_accepted: *mut u32,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_frames_accepted.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_frames_accepted");
                    }
                    let channels = channels.max(1);
                    let sample_len = (frames as usize).saturating_mul(channels as usize);
                    let sample_slice: &[f32] = if sample_len == 0 {
                        &[]
                    } else {
                        if samples.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null samples");
                        }
                        unsafe { core::slice::from_raw_parts(samples, sample_len) }
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::instance::OutputSinkInstance>::write_interleaved_f32(&mut boxed.inner, channels, sample_slice) {
                        Ok(accepted) => {
                            unsafe { *out_frames_accepted = accepted.min(frames); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, e),
                    }
                }

                extern "C" fn flush(handle: *mut core::ffi::c_void) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::instance::OutputSinkInstance>::flush(&mut boxed.inner) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, e),
                    }
                }

                extern "C" fn close(handle: *mut core::ffi::c_void) {
                    if handle.is_null() {
                        return;
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    let _ = <SinkImpl as $crate::instance::OutputSinkInstance>::close(&mut boxed.inner);
                }

                extern "C" fn plan_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_plan: *mut stellatune_plugin_api::StConfigUpdatePlan,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_plan.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_plan");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::update::ConfigUpdatable>::plan_config_update_json(&boxed.inner, new_json) {
                        Ok(plan) => match $crate::update::write_plan_to_ffi(out_plan, plan) {
                            Ok(()) => $crate::status_ok(),
                            Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                        },
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn apply_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::update::ConfigUpdatable>::apply_config_update_json(&mut boxed.inner, new_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn export_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_json_utf8.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::update::ConfigUpdatable>::export_state_json(&boxed.inner) {
                        Ok(Some(json)) => {
                            unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                            $crate::status_ok()
                        }
                        Ok(None) => {
                            unsafe { *out_json_utf8 = $crate::StStr::empty(); }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn import_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    state_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    if handle.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                    }
                    let state_json = match unsafe { $crate::ststr_to_str(&state_json_utf8) } {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let boxed = unsafe { &mut *(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>) };
                    match <SinkImpl as $crate::update::ConfigUpdatable>::import_state_json(&mut boxed.inner, state_json) {
                        Ok(()) => $crate::status_ok(),
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }

                extern "C" fn destroy(handle: *mut core::ffi::c_void) {
                    if handle.is_null() {
                        return;
                    }
                    unsafe { drop(Box::from_raw(handle as *mut $crate::instance::OutputSinkBox<SinkImpl>)); }
                }

                pub static VTABLE: stellatune_plugin_api::StOutputSinkInstanceVTable =
                    stellatune_plugin_api::StOutputSinkInstanceVTable {
                        list_targets_json_utf8,
                        negotiate_spec,
                        open,
                        write_interleaved_f32,
                        flush: Some(flush),
                        close,
                        plan_config_update_json_utf8: Some(plan_config_update_json_utf8),
                        apply_config_update_json_utf8: Some(apply_config_update_json_utf8),
                        export_state_json_utf8: Some(export_state_json_utf8),
                        import_state_json_utf8: Some(import_state_json_utf8),
                        destroy,
                    };

                pub extern "C" fn create_instance(
                    config_json_utf8: $crate::StStr,
                    out_instance: *mut stellatune_plugin_api::StOutputSinkInstanceRef,
                ) -> $crate::StStatus {
                    if out_instance.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_instance");
                    }
                    let json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$sink_ty as $crate::instance::OutputSinkDescriptor>::Config>(json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e.to_string()),
                    };
                    match <$sink_ty as $crate::instance::OutputSinkDescriptor>::create(config) {
                        Ok(instance) => {
                            let boxed = Box::new($crate::instance::OutputSinkBox { inner: instance });
                            unsafe {
                                *out_instance = stellatune_plugin_api::StOutputSinkInstanceRef {
                                    handle: Box::into_raw(boxed) as *mut core::ffi::c_void,
                                    vtable: &VTABLE as *const _,
                                    reserved0: 0,
                                    reserved1: 0,
                                };
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                    }
                }
            }
        )*

        const __ST_DECODER_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dec_ty>(); 1 })*;
        const __ST_DSP_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$dsp_ty>(); 1 })*;
        const __ST_SOURCE_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$source_ty>(); 1 })*;
        const __ST_LYRICS_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$lyrics_ty>(); 1 })*;
        const __ST_SINK_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$sink_ty>(); 1 })*;
        const __ST_CAPABILITY_COUNT: usize = __ST_DECODER_COUNT
            + __ST_DSP_COUNT
            + __ST_SOURCE_COUNT
            + __ST_LYRICS_COUNT
            + __ST_SINK_COUNT;

        extern "C" fn __st_capability_count() -> usize {
            __ST_CAPABILITY_COUNT
        }

        extern "C" fn __st_capability_get(
            index: usize,
        ) -> *const stellatune_plugin_api::StCapabilityDescriptor {
            let mut i = 0usize;
            $(
                if index == i {
                    return &$dec_mod::CAP_DESC as *const _;
                }
                i += 1;
            )*
            $(
                if index == i {
                    return &$dsp_mod::CAP_DESC as *const _;
                }
                i += 1;
            )*
            $(
                if index == i {
                    return &$source_mod::CAP_DESC as *const _;
                }
                i += 1;
            )*
            $(
                if index == i {
                    return &$lyrics_mod::CAP_DESC as *const _;
                }
                i += 1;
            )*
            $(
                if index == i {
                    return &$sink_mod::CAP_DESC as *const _;
                }
                i += 1;
            )*
            core::ptr::null()
        }

        extern "C" fn __st_decoder_ext_score_count(type_id_utf8: $crate::StStr) -> usize {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(_) => return 0,
            };
            $(
                if type_id == <$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID {
                    return $dec_mod::ext_score_count();
                }
            )*
            0
        }

        extern "C" fn __st_decoder_ext_score_get(
            type_id_utf8: $crate::StStr,
            index: usize,
        ) -> *const stellatune_plugin_api::StDecoderExtScore {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(_) => return core::ptr::null(),
            };
            $(
                if type_id == <$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID {
                    return $dec_mod::ext_score_get(index);
                }
            )*
            core::ptr::null()
        }

        extern "C" fn __st_create_decoder_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StDecoderInstanceRef,
        ) -> $crate::StStatus {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
            };
            $(
                if type_id == <$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID {
                    return $dec_mod::create_instance(config_json_utf8, out_instance);
                }
            )*
            $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "decoder type unsupported")
        }

        extern "C" fn __st_create_dsp_instance(
            type_id_utf8: $crate::StStr,
            sample_rate: u32,
            channels: u16,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StDspInstanceRef,
        ) -> $crate::StStatus {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
            };
            $(
                if type_id == <$dsp_ty as $crate::instance::DspDescriptor>::TYPE_ID {
                    return $dsp_mod::create_instance(sample_rate, channels, config_json_utf8, out_instance);
                }
            )*
            $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "dsp type unsupported")
        }

        extern "C" fn __st_create_source_catalog_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StSourceCatalogInstanceRef,
        ) -> $crate::StStatus {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
            };
            $(
                if type_id == <$source_ty as $crate::instance::SourceCatalogDescriptor>::TYPE_ID {
                    return $source_mod::create_instance(config_json_utf8, out_instance);
                }
            )*
            $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "source catalog type unsupported")
        }

        extern "C" fn __st_create_lyrics_provider_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StLyricsProviderInstanceRef,
        ) -> $crate::StStatus {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
            };
            $(
                if type_id == <$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::TYPE_ID {
                    return $lyrics_mod::create_instance(config_json_utf8, out_instance);
                }
            )*
            $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "lyrics provider type unsupported")
        }

        extern "C" fn __st_create_output_sink_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_instance: *mut stellatune_plugin_api::StOutputSinkInstanceRef,
        ) -> $crate::StStatus {
            let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                Ok(s) => s,
                Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
            };
            $(
                if type_id == <$sink_ty as $crate::instance::OutputSinkDescriptor>::TYPE_ID {
                    return $sink_mod::create_instance(config_json_utf8, out_instance);
                }
            )*
            $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "output sink type unsupported")
        }

        static __ST_PLUGIN_MODULE_FULL: stellatune_plugin_api::StPluginModule =
            stellatune_plugin_api::StPluginModule {
                api_version: stellatune_plugin_api::STELLATUNE_PLUGIN_API_VERSION,
                plugin_version: $crate::StVersion {
                    major: $vmaj,
                    minor: $vmin,
                    patch: $vpatch,
                    reserved: 0,
                },
                plugin_free: Some($crate::plugin_free),
                metadata_json_utf8: __st_plugin_metadata_json_utf8,
                capability_count: __st_capability_count,
                capability_get: __st_capability_get,
                decoder_ext_score_count: Some(__st_decoder_ext_score_count),
                decoder_ext_score_get: Some(__st_decoder_ext_score_get),
                create_decoder_instance: Some(__st_create_decoder_instance),
                create_dsp_instance: Some(__st_create_dsp_instance),
                create_source_catalog_instance: Some(__st_create_source_catalog_instance),
                create_lyrics_provider_instance: Some(__st_create_lyrics_provider_instance),
                create_output_sink_instance: Some(__st_create_output_sink_instance),
                shutdown: None,
            };

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn stellatune_plugin_entry(
            host: *const stellatune_plugin_api::StHostVTable,
        ) -> *const stellatune_plugin_api::StPluginModule {
            unsafe { $crate::export::__set_host_vtable(host) };
            &__ST_PLUGIN_MODULE_FULL
        }
    };
}
