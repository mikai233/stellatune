#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_output_modules {
    ($( $sink_mod:ident => $sink_ty:ty ),* $(,)?) => {
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
                        Ok(plan) => match unsafe { $crate::update::write_plan_to_ffi(out_plan, plan) } {
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
    };
}
