#![allow(clippy::wildcard_imports)] // Intentional wildcard usage (API facade, macro template, or generated code).

#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_source_modules {
    ($( $source_mod:ident => $source_ty:ty ),* $(,)?) => {
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
    };
}
