/// Export multiple `StOutputSinkVTableV1` entries through a single
/// `StOutputSinkRegistryV1` interface.
///
/// Example:
/// ```ignore
/// stellatune_plugin_sdk::export_output_sinks_interface! {
///   sinks: [
///     asio => AsioOutputSink,
///     virtual_device => VirtualSink,
///   ],
/// }
///
/// stellatune_plugin_sdk::compose_get_interface! {
///   fn __st_get_interface;
///   __st_output_sinks_get_interface,
/// }
///
/// stellatune_plugin_sdk::export_plugin! {
///   id: "dev.stellatune.output.multi",
///   name: "Multi Output",
///   version: (0, 1, 0),
///   decoders: [],
///   dsps: [],
///   get_interface: __st_get_interface,
/// }
/// ```
#[macro_export]
macro_rules! export_output_sinks_interface {
    (
        sinks: [
            $($sink_mod:ident => $sink_ty:ty),* $(,)?
        ]
        $(, fallback_get_interface: $fallback_get_interface:path)?
        $(,)?
    ) => {
        $(
            mod $sink_mod {
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
                    static DEFAULT_CONFIG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                    let s = DEFAULT_CONFIG.get_or_init(|| {
                        $crate::__private::serde_json::to_string(
                            &<$sink_ty as $crate::OutputSinkDescriptor>::default_config(),
                        )
                        .unwrap_or_else(|_| "{}".to_string())
                    });
                    $crate::StStr {
                        ptr: s.as_ptr(),
                        len: s.len(),
                    }
                }

                extern "C" fn list_targets_json_utf8(
                    config_json_utf8: $crate::StStr,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if out_json_utf8.is_null() {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null out_json_utf8",
                        );
                    }
                    let config_json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$sink_ty as $crate::OutputSinkDescriptor>::Config>(config_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    match <$sink_ty as $crate::OutputSinkDescriptor>::list_targets(&config) {
                        Ok(targets) => {
                            let json = match $crate::__private::serde_json::to_string(&targets) {
                                Ok(v) => v,
                                Err(e) => return $crate::status_err_msg($crate::ST_ERR_INTERNAL, &e.to_string()),
                            };
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
                    let config = match $crate::__private::serde_json::from_str::<<$sink_ty as $crate::OutputSinkDescriptor>::Config>(config_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    let target_json = match unsafe { $crate::ststr_to_str(&target_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let target = match $crate::__private::serde_json::from_str::<<$sink_ty as $crate::OutputSinkDescriptor>::Target>(target_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    match <$sink_ty as $crate::OutputSinkDescriptor>::open(spec, &config, &target)
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

                extern "C" fn negotiate_spec(
                    config_json_utf8: $crate::StStr,
                    target_json_utf8: $crate::StStr,
                    desired_spec: $crate::StAudioSpec,
                    out_negotiated: *mut $crate::StOutputSinkNegotiatedSpecV1,
                ) -> $crate::StStatus {
                    if out_negotiated.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_negotiated");
                    }
                    let config_json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$sink_ty as $crate::OutputSinkDescriptor>::Config>(config_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    let target_json = match unsafe { $crate::ststr_to_str(&target_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let target = match $crate::__private::serde_json::from_str::<<$sink_ty as $crate::OutputSinkDescriptor>::Target>(target_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };

                    match <$sink_ty as $crate::OutputSinkDescriptor>::negotiate_spec(
                        desired_spec,
                        &config,
                        &target,
                    ) {
                        Ok(mut negotiated) => {
                            negotiated.spec.sample_rate = negotiated.spec.sample_rate.max(1);
                            negotiated.spec.channels = negotiated.spec.channels.max(1);
                            negotiated.spec.reserved = 0;
                            unsafe {
                                *out_negotiated = negotiated;
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
                    let boxed = unsafe { &mut *(handle as *mut $crate::OutputSinkBox<$sink_ty>) };
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
                    let boxed = unsafe { &mut *(handle as *mut $crate::OutputSinkBox<$sink_ty>) };
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
                        drop(Box::from_raw(handle as *mut $crate::OutputSinkBox<$sink_ty>));
                    }
                }

                pub(super) static VTABLE: $crate::StOutputSinkVTableV1 = $crate::StOutputSinkVTableV1 {
                    type_id_utf8,
                    display_name_utf8,
                    config_schema_json_utf8,
                    default_config_json_utf8,
                    list_targets_json_utf8,
                    negotiate_spec,
                    open,
                    write_interleaved_f32,
                    flush: Some(flush),
                    close,
                };
            }
        )*

        const __ST_OUTPUT_SINK_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$sink_ty>(); 1 })*;

        extern "C" fn __st_output_sink_count() -> usize {
            __ST_OUTPUT_SINK_COUNT
        }

        extern "C" fn __st_output_sink_get(index: usize) -> *const $crate::StOutputSinkVTableV1 {
            let vtables = [$( &$sink_mod::VTABLE as *const $crate::StOutputSinkVTableV1 ),*];
            vtables.get(index).copied().unwrap_or(core::ptr::null())
        }

        static __ST_OUTPUT_SINK_REGISTRY: $crate::StOutputSinkRegistryV1 =
            $crate::StOutputSinkRegistryV1 {
                output_sink_count: __st_output_sink_count,
                output_sink_get: __st_output_sink_get,
            };

        extern "C" fn __st_output_sinks_get_interface(
            interface_id_utf8: $crate::StStr,
        ) -> *const core::ffi::c_void {
            let interface_id = match unsafe { $crate::ststr_to_str(&interface_id_utf8) } {
                Ok(s) => s,
                Err(_) => "",
            };
            if interface_id == $crate::ST_INTERFACE_OUTPUT_SINKS_V1 {
                return &__ST_OUTPUT_SINK_REGISTRY as *const $crate::StOutputSinkRegistryV1
                    as *const core::ffi::c_void;
            }
            $(
                return $fallback_get_interface(interface_id_utf8);
            )?
            core::ptr::null()
        }
    };
}
