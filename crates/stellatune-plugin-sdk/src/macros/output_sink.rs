/// Export a `StOutputSinkVTableV1` interface from an [`OutputSinkDescriptor`] type.
///
/// Example:
/// ```ignore
/// stellatune_plugin_sdk::export_output_sink_interface! {
///   sink: AsioOutputSink,
/// }
///
/// stellatune_plugin_sdk::export_plugin! {
///   id: "dev.stellatune.output.asio",
///   name: "ASIO Output",
///   version: (0, 1, 0),
///   decoders: [],
///   dsps: [],
///   get_interface: __st_output_sink_get_interface,
/// }
/// ```
#[macro_export]
macro_rules! export_output_sink_interface {
    (
        sink: $sink_ty:ty
        $(, fallback_get_interface: $fallback_get_interface:path)?
        $(,)?
    ) => {
        mod __st_output_sink_mod {
            use super::*;

            extern "C" fn type_id_utf8() -> $crate::StStr {
                $crate::ststr(<$sink_ty as $crate::OutputSinkDescriptor>::TYPE_ID)
            }

            extern "C" fn display_name_utf8() -> $crate::StStr {
                $crate::ststr(<$sink_ty as $crate::OutputSinkDescriptor>::DISPLAY_NAME)
            }

            extern "C" fn config_schema_json_utf8() -> $crate::StStr {
                $crate::ststr(<$sink_ty as $crate::OutputSinkDescriptor>::CONFIG_SCHEMA_JSON)
            }

            extern "C" fn default_config_json_utf8() -> $crate::StStr {
                $crate::ststr(<$sink_ty as $crate::OutputSinkDescriptor>::DEFAULT_CONFIG_JSON)
            }

            extern "C" fn list_targets_json_utf8(
                config_json_utf8: $crate::StStr,
                out_json_utf8: *mut $crate::StStr,
            ) -> $crate::StStatus {
                if out_json_utf8.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_json_utf8");
                }
                let config_json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                };
                match <$sink_ty as $crate::OutputSinkDescriptor>::list_targets_json(config_json) {
                    Ok(json) => {
                        unsafe {
                            *out_json_utf8 = $crate::alloc_utf8_bytes(&json);
                        }
                        $crate::status_ok()
                    }
                    Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, &e),
                }
            }

            extern "C" fn open(
                config_json_utf8: $crate::StStr,
                target_json_utf8: $crate::StStr,
                spec: $crate::StAudioSpec,
                out_handle: *mut *mut core::ffi::c_void,
            ) -> $crate::StStatus {
                if out_handle.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_handle");
                }
                let config_json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                };
                let target_json = match unsafe { $crate::ststr_to_str(&target_json_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                };
                match <$sink_ty as $crate::OutputSinkDescriptor>::open(spec, config_json, target_json)
                {
                    Ok(sink) => {
                        let boxed = Box::new($crate::OutputSinkBox { inner: sink });
                        unsafe {
                            *out_handle = Box::into_raw(boxed) as *mut core::ffi::c_void;
                        }
                        $crate::status_ok()
                    }
                    Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, &e),
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
                    return $crate::status_err_msg(
                        $crate::ST_ERR_INVALID_ARG,
                        "null handle/out_frames_accepted",
                    );
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
                let boxed =
                    unsafe { &mut *(handle as *mut $crate::OutputSinkBox<$sink_ty>) };
                match <$sink_ty as $crate::OutputSink>::write_interleaved_f32(
                    &mut boxed.inner,
                    channels,
                    sample_slice,
                ) {
                    Ok(accepted) => {
                        unsafe {
                            *out_frames_accepted = accepted.min(frames);
                        }
                        $crate::status_ok()
                    }
                    Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, &e),
                }
            }

            extern "C" fn flush(handle: *mut core::ffi::c_void) -> $crate::StStatus {
                if handle.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                }
                let boxed =
                    unsafe { &mut *(handle as *mut $crate::OutputSinkBox<$sink_ty>) };
                match <$sink_ty as $crate::OutputSink>::flush(&mut boxed.inner) {
                    Ok(()) => $crate::status_ok(),
                    Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, &e),
                }
            }

            extern "C" fn close(handle: *mut core::ffi::c_void) {
                if handle.is_null() {
                    return;
                }
                unsafe {
                    drop(Box::from_raw(
                        handle as *mut $crate::OutputSinkBox<$sink_ty>,
                    ));
                }
            }

            pub(super) static VTABLE: $crate::StOutputSinkVTableV1 = $crate::StOutputSinkVTableV1 {
                type_id_utf8,
                display_name_utf8,
                config_schema_json_utf8,
                default_config_json_utf8,
                list_targets_json_utf8,
                open,
                write_interleaved_f32,
                flush: Some(flush),
                close,
            };
        }

        extern "C" fn __st_output_sink_get_interface(
            interface_id_utf8: $crate::StStr,
        ) -> *const core::ffi::c_void {
            let interface_id = match unsafe { $crate::ststr_to_str(&interface_id_utf8) } {
                Ok(s) => s,
                Err(_) => "",
            };
            if interface_id == $crate::ST_INTERFACE_OUTPUT_SINK_V1 {
                return &__st_output_sink_mod::VTABLE as *const $crate::StOutputSinkVTableV1
                    as *const core::ffi::c_void;
            }
            $(
                return $fallback_get_interface(interface_id_utf8);
            )?
            core::ptr::null()
        }
    };
}
