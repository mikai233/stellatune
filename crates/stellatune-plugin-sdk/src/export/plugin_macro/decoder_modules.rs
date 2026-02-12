#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_decoder_modules {
    ($( $dec_mod:ident => $dec_ty:ty ),* $(,)?) => {
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
                    $crate::ffi_guard::guard_with_default("ext_score_count", 0, || {
                        ext_score_rules_ffi().len()
                    })
                }

                pub extern "C" fn ext_score_get(
                    index: usize,
                ) -> *const stellatune_plugin_api::StDecoderExtScore {
                    $crate::ffi_guard::guard_with_default("ext_score_get", core::ptr::null(), || {
                        ext_score_rules_ffi()
                            .get(index)
                            .map(|v| v as *const _)
                            .unwrap_or(core::ptr::null())
                    })
                }

                extern "C" fn open(
                    handle: *mut core::ffi::c_void,
                    args: stellatune_plugin_api::StDecoderOpenArgs,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("open", || {
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
                    })
                }

                extern "C" fn get_info(
                    handle: *mut core::ffi::c_void,
                    out_info: *mut $crate::StDecoderInfo,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("get_info", || {
                        if handle.is_null() || out_info.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_info");
                        }
                        let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                        let info = <$dec_ty as $crate::instance::DecoderInstance>::get_info(&boxed.inner);
                        boxed.channels = info.spec.channels.max(1);
                        unsafe { *out_info = info; }
                        $crate::status_ok()
                    })
                }

                extern "C" fn get_metadata_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("get_metadata_json_utf8", || {
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
                    })
                }

                extern "C" fn read_interleaved_f32(
                    handle: *mut core::ffi::c_void,
                    frames: u32,
                    out_interleaved: *mut f32,
                    out_frames_read: *mut u32,
                    out_eof: *mut bool,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("read_interleaved_f32", || {
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
                    })
                }

                extern "C" fn seek_ms(
                    handle: *mut core::ffi::c_void,
                    position_ms: u64,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("seek_ms", || {
                        if handle.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                        }
                        let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                        match <$dec_ty as $crate::instance::DecoderInstance>::seek_ms(&mut boxed.inner, position_ms) {
                            Ok(()) => $crate::status_ok(),
                            Err(e) => $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, e),
                        }
                    })
                }

                extern "C" fn plan_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_plan: *mut stellatune_plugin_api::StConfigUpdatePlan,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("plan_config_update_json_utf8", || {
                        if handle.is_null() || out_plan.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_plan");
                        }
                        let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                            Ok(v) => v,
                            Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                        };
                        let boxed = unsafe { &mut *(handle as *mut $crate::instance::DecoderBox<$dec_ty>) };
                        match <$dec_ty as $crate::update::ConfigUpdatable>::plan_config_update_json(&boxed.inner, new_json) {
                            Ok(plan) => match unsafe { $crate::update::write_plan_to_ffi(out_plan, plan) } {
                                Ok(()) => $crate::status_ok(),
                                Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                            },
                            Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                        }
                    })
                }

                extern "C" fn apply_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("apply_config_update_json_utf8", || {
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
                    })
                }

                extern "C" fn export_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("export_state_json_utf8", || {
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
                    })
                }

                extern "C" fn import_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    state_json_utf8: $crate::StStr,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("import_state_json_utf8", || {
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
                    })
                }

                extern "C" fn destroy(handle: *mut core::ffi::c_void) {
                    $crate::ffi_guard::guard_void("destroy", || {
                        if handle.is_null() {
                            return;
                        }
                        unsafe { drop(Box::from_raw(handle as *mut $crate::instance::DecoderBox<$dec_ty>)); }
                    });
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
                    $crate::ffi_guard::guard_status("create_instance", || {
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
                    })
                }
            }
        )*
    };
}
