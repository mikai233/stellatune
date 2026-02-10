#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_lyrics_modules {
    ($( $lyrics_mod:ident => $lyrics_ty:ty ),* $(,)?) => {
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
    };
}
