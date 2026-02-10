#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_dsp_modules {
    ($( $dsp_mod:ident => $dsp_ty:ty ),* $(,)?) => {
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
    };
}
