/// Export multiple source catalogs through a single `StSourceCatalogRegistryV1` interface.
///
/// High-level mode (recommended):
/// ```ignore
/// stellatune_plugin_sdk::export_source_catalogs_interface! {
///   sources: [
///     local => LocalSourceCatalog,
///     remote => RemoteSourceCatalog,
///   ],
/// }
/// ```
///
/// Low-level mode:
/// ```ignore
/// static LOCAL_SOURCE_VTABLE: stellatune_plugin_sdk::StSourceCatalogVTableV1 = /* ... */;
/// static REMOTE_SOURCE_VTABLE: stellatune_plugin_sdk::StSourceCatalogVTableV1 = /* ... */;
///
/// stellatune_plugin_sdk::export_source_catalogs_interface! {
///   sources: [
///     LOCAL_SOURCE_VTABLE,
///     REMOTE_SOURCE_VTABLE,
///   ],
/// }
/// ```
///
/// For composing multiple interface exporters, prefer:
/// `stellatune_plugin_sdk::compose_get_interface!`.
#[macro_export]
macro_rules! export_source_catalogs_interface {
    (
        sources: [
            $($source_mod:ident => $source_ty:ty),* $(,)?
        ]
        $(, fallback_get_interface: $fallback_get_interface:path)?
        $(,)?
    ) => {
        $(
            mod $source_mod {
                use super::*;

                type StreamImpl = <$source_ty as $crate::SourceCatalogDescriptor>::Stream;

                extern "C" fn type_id_utf8() -> $crate::StStr {
                    $crate::ststr(<$source_ty as $crate::SourceCatalogDescriptor>::TYPE_ID)
                }

                extern "C" fn display_name_utf8() -> $crate::StStr {
                    $crate::ststr(<$source_ty as $crate::SourceCatalogDescriptor>::DISPLAY_NAME)
                }

                extern "C" fn config_schema_json_utf8() -> $crate::StStr {
                    $crate::ststr(<$source_ty as $crate::SourceCatalogDescriptor>::CONFIG_SCHEMA_JSON)
                }

                extern "C" fn default_config_json_utf8() -> $crate::StStr {
                    static DEFAULT_CONFIG: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                    let s = DEFAULT_CONFIG.get_or_init(|| {
                        $crate::__private::serde_json::to_string(
                            &<$source_ty as $crate::SourceCatalogDescriptor>::default_config(),
                        )
                        .unwrap_or_else(|_| "{}".to_string())
                    });
                    $crate::StStr {
                        ptr: s.as_ptr(),
                        len: s.len(),
                    }
                }

                extern "C" fn list_items_json_utf8(
                    config_json_utf8: $crate::StStr,
                    request_json_utf8: $crate::StStr,
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
                    let config = match $crate::__private::serde_json::from_str::<<$source_ty as $crate::SourceCatalogDescriptor>::Config>(config_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    let request_json = match unsafe { $crate::ststr_to_str(&request_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let request = match $crate::__private::serde_json::from_str::<<$source_ty as $crate::SourceCatalogDescriptor>::ListRequest>(request_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    match <$source_ty as $crate::SourceCatalogDescriptor>::list_items(
                        &config,
                        &request,
                    ) {
                        Ok(items) => {
                            let json = match $crate::__private::serde_json::to_string(&items) {
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
                    let boxed = unsafe { &mut *(handle as *mut $crate::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::SourceStream>::read(&mut boxed.inner, out_slice) {
                        Ok(n) => {
                            unsafe {
                                *out_read = n.min(len);
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, &e),
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
                    let boxed = unsafe { &mut *(handle as *mut $crate::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::SourceStream>::seek(&mut boxed.inner, offset, whence) {
                        Ok(pos) => {
                            unsafe {
                                *out_pos = pos;
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, &e),
                    }
                }

                extern "C" fn io_tell(
                    handle: *mut core::ffi::c_void,
                    out_pos: *mut u64,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_pos.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "invalid io_tell args");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::SourceStream>::tell(&mut boxed.inner) {
                        Ok(pos) => {
                            unsafe {
                                *out_pos = pos;
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, &e),
                    }
                }

                extern "C" fn io_size(
                    handle: *mut core::ffi::c_void,
                    out_size: *mut u64,
                ) -> $crate::StStatus {
                    if handle.is_null() || out_size.is_null() {
                        return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "invalid io_size args");
                    }
                    let boxed = unsafe { &mut *(handle as *mut $crate::SourceStreamBox<StreamImpl>) };
                    match <StreamImpl as $crate::SourceStream>::size(&mut boxed.inner) {
                        Ok(size) => {
                            unsafe {
                                *out_size = size;
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_IO, &e),
                    }
                }

                extern "C" fn open_stream(
                    config_json_utf8: $crate::StStr,
                    track_json_utf8: $crate::StStr,
                    out_io_vtable: *mut *const $crate::StIoVTableV1,
                    out_io_handle: *mut *mut core::ffi::c_void,
                    out_track_meta_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    if out_io_vtable.is_null()
                        || out_io_handle.is_null()
                        || out_track_meta_json_utf8.is_null()
                    {
                        return $crate::status_err_msg(
                            $crate::ST_ERR_INVALID_ARG,
                            "null out_io_vtable/out_io_handle/out_track_meta_json_utf8",
                        );
                    }
                    let config_json = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let config = match $crate::__private::serde_json::from_str::<<$source_ty as $crate::SourceCatalogDescriptor>::Config>(config_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    let track_json = match unsafe { $crate::ststr_to_str(&track_json_utf8) } {
                        Ok(s) => s,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e),
                    };
                    let track = match $crate::__private::serde_json::from_str::<<$source_ty as $crate::SourceCatalogDescriptor>::Track>(track_json) {
                        Ok(v) => v,
                        Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, &e.to_string()),
                    };
                    match <$source_ty as $crate::SourceCatalogDescriptor>::open_stream(
                        &config,
                        &track,
                    ) {
                        Ok(opened) => {
                            let $crate::SourceOpenResult {
                                stream,
                                track_meta,
                            } = opened;
                            let boxed = Box::new($crate::SourceStreamBox { inner: stream });
                            unsafe {
                                *out_io_vtable = &IO_VTABLE as *const $crate::StIoVTableV1;
                                *out_io_handle = Box::into_raw(boxed) as *mut core::ffi::c_void;
                            }
                            let track_meta_json = match track_meta {
                                Some(meta) => match $crate::__private::serde_json::to_string(&meta) {
                                    Ok(v) => Some(v),
                                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INTERNAL, &e.to_string()),
                                },
                                None => None,
                            };
                            unsafe {
                                *out_track_meta_json_utf8 = track_meta_json
                                    .as_deref()
                                    .map($crate::alloc_utf8_bytes)
                                    .unwrap_or_else($crate::StStr::empty);
                            }
                            $crate::status_ok()
                        }
                        Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, &e),
                    }
                }

                extern "C" fn close_stream(io_handle: *mut core::ffi::c_void) {
                    if io_handle.is_null() {
                        return;
                    }
                    unsafe {
                        drop(Box::from_raw(io_handle as *mut $crate::SourceStreamBox<StreamImpl>));
                    }
                }

                pub(super) static IO_VTABLE: $crate::StIoVTableV1 = $crate::StIoVTableV1 {
                    read: io_read,
                    seek: if <StreamImpl as $crate::SourceStream>::SUPPORTS_SEEK {
                        Some(io_seek)
                    } else {
                        None
                    },
                    tell: if <StreamImpl as $crate::SourceStream>::SUPPORTS_TELL {
                        Some(io_tell)
                    } else {
                        None
                    },
                    size: if <StreamImpl as $crate::SourceStream>::SUPPORTS_SIZE {
                        Some(io_size)
                    } else {
                        None
                    },
                };

                pub(super) static VTABLE: $crate::StSourceCatalogVTableV1 = $crate::StSourceCatalogVTableV1 {
                    type_id_utf8,
                    display_name_utf8,
                    config_schema_json_utf8,
                    default_config_json_utf8,
                    list_items_json_utf8,
                    open_stream,
                    close_stream,
                };
            }
        )*

        const __ST_SOURCE_CATALOG_COUNT: usize = 0 $(+ { let _ = core::mem::size_of::<$source_ty>(); 1 })*;

        extern "C" fn __st_source_catalog_count() -> usize {
            __ST_SOURCE_CATALOG_COUNT
        }

        extern "C" fn __st_source_catalog_get(
            index: usize,
        ) -> *const $crate::StSourceCatalogVTableV1 {
            let vtables = [$( &$source_mod::VTABLE as *const $crate::StSourceCatalogVTableV1 ),*];
            vtables.get(index).copied().unwrap_or(core::ptr::null())
        }

        static __ST_SOURCE_CATALOG_REGISTRY: $crate::StSourceCatalogRegistryV1 =
            $crate::StSourceCatalogRegistryV1 {
                source_catalog_count: __st_source_catalog_count,
                source_catalog_get: __st_source_catalog_get,
            };

        extern "C" fn __st_source_catalogs_get_interface(
            interface_id_utf8: $crate::StStr,
        ) -> *const core::ffi::c_void {
            let interface_id = match unsafe { $crate::ststr_to_str(&interface_id_utf8) } {
                Ok(s) => s,
                Err(_) => "",
            };
            if interface_id == $crate::ST_INTERFACE_SOURCE_CATALOGS_V1 {
                return &__ST_SOURCE_CATALOG_REGISTRY as *const $crate::StSourceCatalogRegistryV1
                    as *const core::ffi::c_void;
            }
            $(
                return $fallback_get_interface(interface_id_utf8);
            )?
            core::ptr::null()
        }
    };
    (
        sources: [
            $($source_vtable:path),* $(,)?
        ]
        $(, fallback_get_interface: $fallback_get_interface:path)?
        $(,)?
    ) => {
        const __ST_SOURCE_CATALOG_COUNT: usize = 0 $(+ { let _ = &$source_vtable; 1 })*;

        extern "C" fn __st_source_catalog_count() -> usize {
            __ST_SOURCE_CATALOG_COUNT
        }

        extern "C" fn __st_source_catalog_get(
            index: usize,
        ) -> *const $crate::StSourceCatalogVTableV1 {
            let vtables = [$( &$source_vtable as *const $crate::StSourceCatalogVTableV1 ),*];
            vtables.get(index).copied().unwrap_or(core::ptr::null())
        }

        static __ST_SOURCE_CATALOG_REGISTRY: $crate::StSourceCatalogRegistryV1 =
            $crate::StSourceCatalogRegistryV1 {
                source_catalog_count: __st_source_catalog_count,
                source_catalog_get: __st_source_catalog_get,
            };

        extern "C" fn __st_source_catalogs_get_interface(
            interface_id_utf8: $crate::StStr,
        ) -> *const core::ffi::c_void {
            let interface_id = match unsafe { $crate::ststr_to_str(&interface_id_utf8) } {
                Ok(s) => s,
                Err(_) => "",
            };
            if interface_id == $crate::ST_INTERFACE_SOURCE_CATALOGS_V1 {
                return &__ST_SOURCE_CATALOG_REGISTRY as *const $crate::StSourceCatalogRegistryV1
                    as *const core::ffi::c_void;
            }
            $(
                return $fallback_get_interface(interface_id_utf8);
            )?
            core::ptr::null()
        }
    };
}
