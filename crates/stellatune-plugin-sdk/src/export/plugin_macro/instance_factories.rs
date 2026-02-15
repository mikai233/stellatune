#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_instance_factories {
    (
        dsps: [$( $dsp_mod:ident => $dsp_ty:ty ),* $(,)?],
        source_catalogs: [$( $source_mod:ident => $source_ty:ty ),* $(,)?],
        lyrics_providers: [$( $lyrics_mod:ident => $lyrics_ty:ty ),* $(,)?],
        output_sinks: [$( $sink_mod:ident => $sink_ty:ty ),* $(,)?],
    ) => {
        struct __StDspInstanceOut(stellatune_plugin_api::StDspInstanceRef);
        unsafe impl Send for __StDspInstanceOut {}
        struct __StSourceInstanceOut(stellatune_plugin_api::StSourceCatalogInstanceRef);
        unsafe impl Send for __StSourceInstanceOut {}
        struct __StLyricsInstanceOut(stellatune_plugin_api::StLyricsProviderInstanceRef);
        unsafe impl Send for __StLyricsInstanceOut {}
        struct __StOutputInstanceOut(stellatune_plugin_api::StOutputSinkInstanceRef);
        unsafe impl Send for __StOutputInstanceOut {}

        type __StCreateDspOp = $crate::async_task::AsyncTaskOp<__StDspInstanceOut>;
        type __StCreateSourceOp = $crate::async_task::AsyncTaskOp<__StSourceInstanceOut>;
        type __StCreateLyricsOp = $crate::async_task::AsyncTaskOp<__StLyricsInstanceOut>;
        type __StCreateOutputOp = $crate::async_task::AsyncTaskOp<__StOutputInstanceOut>;

        fn __st_async_take_error_to_status(
            op_name: &str,
            err: $crate::async_task::AsyncTaskTakeError,
        ) -> $crate::StStatus {
            match err {
                $crate::async_task::AsyncTaskTakeError::Pending => {
                    $crate::status_err_msg($crate::ST_ERR_INTERNAL, format!("{op_name} operation not ready"))
                }
                $crate::async_task::AsyncTaskTakeError::Cancelled => {
                    $crate::status_err_msg($crate::ST_ERR_INTERNAL, format!("{op_name} operation cancelled"))
                }
                $crate::async_task::AsyncTaskTakeError::Failed(msg) => {
                    if msg.is_empty() {
                        $crate::status_err_msg($crate::ST_ERR_INTERNAL, format!("{op_name} operation failed"))
                    } else {
                        $crate::status_err_msg($crate::ST_ERR_INTERNAL, msg)
                    }
                }
                $crate::async_task::AsyncTaskTakeError::AlreadyTaken => {
                    $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, format!("{op_name} result already taken"))
                }
            }
        }

        fn __st_status_to_sdk_result(status: $crate::StStatus, what: &str) -> $crate::SdkResult<()> {
            if status.code == 0 {
                return Ok(());
            }
            let message = unsafe { $crate::ststr_to_str(&status.message) }
                .unwrap_or("operation failed")
                .to_string();
            if !status.message.ptr.is_null() && status.message.len > 0 {
                $crate::plugin_free(status.message.ptr as *mut core::ffi::c_void, status.message.len, 1);
            }
            Err($crate::SdkError::msg(format!("{what}: {message}")))
        }

        macro_rules! __st_define_create_op_common {
            (
                $poll_fn:ident,
                $wait_fn:ident,
                $cancel_fn:ident,
                $set_notifier_fn:ident,
                $destroy_fn:ident,
                $op_ty:ty,
                $label:literal
            ) => {
                extern "C" fn $poll_fn(
                    handle: *mut core::ffi::c_void,
                    out_state: *mut stellatune_plugin_api::StAsyncOpState,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status($label, || {
                        if handle.is_null() || out_state.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_state");
                        }
                        let op = unsafe { &*(handle as *mut $op_ty) };
                        unsafe { *out_state = op.poll(); }
                        $crate::status_ok()
                    })
                }

                extern "C" fn $wait_fn(
                    handle: *mut core::ffi::c_void,
                    timeout_ms: u32,
                    out_state: *mut stellatune_plugin_api::StAsyncOpState,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status($label, || {
                        if handle.is_null() || out_state.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_state");
                        }
                        let op = unsafe { &*(handle as *mut $op_ty) };
                        unsafe { *out_state = op.wait(timeout_ms); }
                        $crate::status_ok()
                    })
                }

                extern "C" fn $cancel_fn(handle: *mut core::ffi::c_void) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status($label, || {
                        if handle.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                        }
                        let op = unsafe { &*(handle as *mut $op_ty) };
                        let _ = op.cancel();
                        $crate::status_ok()
                    })
                }

                extern "C" fn $set_notifier_fn(
                    handle: *mut core::ffi::c_void,
                    notifier: stellatune_plugin_api::StOpNotifier,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status($label, || {
                        if handle.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                        }
                        let op = unsafe { &*(handle as *mut $op_ty) };
                        op.set_notifier(notifier);
                        $crate::status_ok()
                    })
                }

                extern "C" fn $destroy_fn(handle: *mut core::ffi::c_void) {
                    $crate::ffi_guard::guard_void($label, || {
                        if handle.is_null() {
                            return;
                        }
                        unsafe { drop(Box::from_raw(handle as *mut $op_ty)); }
                    });
                }
            };
        }

        __st_define_create_op_common!(
            __st_create_dsp_op_poll,
            __st_create_dsp_op_wait,
            __st_create_dsp_op_cancel,
            __st_create_dsp_op_set_notifier,
            __st_create_dsp_op_destroy,
            __StCreateDspOp,
            "__st_create_dsp_op"
        );
        __st_define_create_op_common!(
            __st_create_source_op_poll,
            __st_create_source_op_wait,
            __st_create_source_op_cancel,
            __st_create_source_op_set_notifier,
            __st_create_source_op_destroy,
            __StCreateSourceOp,
            "__st_create_source_op"
        );
        __st_define_create_op_common!(
            __st_create_lyrics_op_poll,
            __st_create_lyrics_op_wait,
            __st_create_lyrics_op_cancel,
            __st_create_lyrics_op_set_notifier,
            __st_create_lyrics_op_destroy,
            __StCreateLyricsOp,
            "__st_create_lyrics_op"
        );
        __st_define_create_op_common!(
            __st_create_output_op_poll,
            __st_create_output_op_wait,
            __st_create_output_op_cancel,
            __st_create_output_op_set_notifier,
            __st_create_output_op_destroy,
            __StCreateOutputOp,
            "__st_create_output_op"
        );

        extern "C" fn __st_create_dsp_op_take_instance(
            handle: *mut core::ffi::c_void,
            out_instance: *mut stellatune_plugin_api::StDspInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_dsp_op_take_instance", || {
                if handle.is_null() || out_instance.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_instance");
                }
                let op = unsafe { &*(handle as *mut __StCreateDspOp) };
                match op.take_result() {
                    Ok(instance) => {
                        unsafe { *out_instance = instance.0; }
                        $crate::status_ok()
                    }
                    Err(err) => __st_async_take_error_to_status("create_dsp_instance", err),
                }
            })
        }

        extern "C" fn __st_create_source_op_take_instance(
            handle: *mut core::ffi::c_void,
            out_instance: *mut stellatune_plugin_api::StSourceCatalogInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_source_op_take_instance", || {
                if handle.is_null() || out_instance.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_instance");
                }
                let op = unsafe { &*(handle as *mut __StCreateSourceOp) };
                match op.take_result() {
                    Ok(instance) => {
                        unsafe { *out_instance = instance.0; }
                        $crate::status_ok()
                    }
                    Err(err) => __st_async_take_error_to_status("create_source_catalog_instance", err),
                }
            })
        }

        extern "C" fn __st_create_lyrics_op_take_instance(
            handle: *mut core::ffi::c_void,
            out_instance: *mut stellatune_plugin_api::StLyricsProviderInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_lyrics_op_take_instance", || {
                if handle.is_null() || out_instance.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_instance");
                }
                let op = unsafe { &*(handle as *mut __StCreateLyricsOp) };
                match op.take_result() {
                    Ok(instance) => {
                        unsafe { *out_instance = instance.0; }
                        $crate::status_ok()
                    }
                    Err(err) => __st_async_take_error_to_status("create_lyrics_provider_instance", err),
                }
            })
        }

        extern "C" fn __st_create_output_op_take_instance(
            handle: *mut core::ffi::c_void,
            out_instance: *mut stellatune_plugin_api::StOutputSinkInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_create_output_op_take_instance", || {
                if handle.is_null() || out_instance.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_instance");
                }
                let op = unsafe { &*(handle as *mut __StCreateOutputOp) };
                match op.take_result() {
                    Ok(instance) => {
                        unsafe { *out_instance = instance.0; }
                        $crate::status_ok()
                    }
                    Err(err) => __st_async_take_error_to_status("create_output_sink_instance", err),
                }
            })
        }

        static __ST_CREATE_DSP_OP_VTABLE: stellatune_plugin_api::StCreateDspInstanceOpVTable =
            stellatune_plugin_api::StCreateDspInstanceOpVTable {
                poll: __st_create_dsp_op_poll,
                wait: __st_create_dsp_op_wait,
                cancel: __st_create_dsp_op_cancel,
                set_notifier: __st_create_dsp_op_set_notifier,
                take_instance: __st_create_dsp_op_take_instance,
                destroy: __st_create_dsp_op_destroy,
            };

        static __ST_CREATE_SOURCE_OP_VTABLE:
            stellatune_plugin_api::StCreateSourceCatalogInstanceOpVTable =
            stellatune_plugin_api::StCreateSourceCatalogInstanceOpVTable {
                poll: __st_create_source_op_poll,
                wait: __st_create_source_op_wait,
                cancel: __st_create_source_op_cancel,
                set_notifier: __st_create_source_op_set_notifier,
                take_instance: __st_create_source_op_take_instance,
                destroy: __st_create_source_op_destroy,
            };

        static __ST_CREATE_LYRICS_OP_VTABLE:
            stellatune_plugin_api::StCreateLyricsProviderInstanceOpVTable =
            stellatune_plugin_api::StCreateLyricsProviderInstanceOpVTable {
                poll: __st_create_lyrics_op_poll,
                wait: __st_create_lyrics_op_wait,
                cancel: __st_create_lyrics_op_cancel,
                set_notifier: __st_create_lyrics_op_set_notifier,
                take_instance: __st_create_lyrics_op_take_instance,
                destroy: __st_create_lyrics_op_destroy,
            };

        static __ST_CREATE_OUTPUT_OP_VTABLE:
            stellatune_plugin_api::StCreateOutputSinkInstanceOpVTable =
            stellatune_plugin_api::StCreateOutputSinkInstanceOpVTable {
                poll: __st_create_output_op_poll,
                wait: __st_create_output_op_wait,
                cancel: __st_create_output_op_cancel,
                set_notifier: __st_create_output_op_set_notifier,
                take_instance: __st_create_output_op_take_instance,
                destroy: __st_create_output_op_destroy,
            };

        extern "C" fn __st_begin_create_dsp_instance(
            type_id_utf8: $crate::StStr,
            sample_rate: u32,
            channels: u16,
            config_json_utf8: $crate::StStr,
            out_op: *mut stellatune_plugin_api::StCreateDspInstanceOpRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_begin_create_dsp_instance", || {
                if out_op.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_op");
                }
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                let config_owned = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                    Ok(s) => s.to_owned(),
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$dsp_ty as $crate::instance::DspDescriptor>::TYPE_ID {
                        let op = __StCreateDspOp::spawn(async move {
                            let mut out_instance = stellatune_plugin_api::StDspInstanceRef {
                                handle: core::ptr::null_mut(),
                                vtable: core::ptr::null(),
                                reserved0: 0,
                                reserved1: 0,
                            };
                            let in_config = $crate::StStr {
                                ptr: config_owned.as_ptr(),
                                len: config_owned.len(),
                            };
                            let status = $dsp_mod::create_instance(
                                sample_rate,
                                channels,
                                in_config,
                                &mut out_instance as *mut _,
                            );
                            __st_status_to_sdk_result(status, "create_dsp_instance")?;
                            Ok(__StDspInstanceOut(out_instance))
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StCreateDspInstanceOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &__ST_CREATE_DSP_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        return $crate::status_ok();
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "dsp type unsupported")
            })
        }

        extern "C" fn __st_begin_create_source_catalog_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_op: *mut stellatune_plugin_api::StCreateSourceCatalogInstanceOpRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_begin_create_source_catalog_instance", || {
                if out_op.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_op");
                }
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                let config_owned = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                    Ok(s) => s.to_owned(),
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$source_ty as $crate::instance::SourceCatalogDescriptor>::TYPE_ID {
                        let op = __StCreateSourceOp::spawn(async move {
                            let mut out_instance = stellatune_plugin_api::StSourceCatalogInstanceRef {
                                handle: core::ptr::null_mut(),
                                vtable: core::ptr::null(),
                                reserved0: 0,
                                reserved1: 0,
                            };
                            let in_config = $crate::StStr {
                                ptr: config_owned.as_ptr(),
                                len: config_owned.len(),
                            };
                            let status = $source_mod::create_instance(in_config, &mut out_instance as *mut _);
                            __st_status_to_sdk_result(status, "create_source_catalog_instance")?;
                            Ok(__StSourceInstanceOut(out_instance))
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StCreateSourceCatalogInstanceOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &__ST_CREATE_SOURCE_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        return $crate::status_ok();
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "source catalog type unsupported")
            })
        }

        extern "C" fn __st_begin_create_lyrics_provider_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_op: *mut stellatune_plugin_api::StCreateLyricsProviderInstanceOpRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_begin_create_lyrics_provider_instance", || {
                if out_op.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_op");
                }
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                let config_owned = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                    Ok(s) => s.to_owned(),
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$lyrics_ty as $crate::instance::LyricsProviderDescriptor>::TYPE_ID {
                        let op = __StCreateLyricsOp::spawn(async move {
                            let mut out_instance = stellatune_plugin_api::StLyricsProviderInstanceRef {
                                handle: core::ptr::null_mut(),
                                vtable: core::ptr::null(),
                                reserved0: 0,
                                reserved1: 0,
                            };
                            let in_config = $crate::StStr {
                                ptr: config_owned.as_ptr(),
                                len: config_owned.len(),
                            };
                            let status = $lyrics_mod::create_instance(in_config, &mut out_instance as *mut _);
                            __st_status_to_sdk_result(status, "create_lyrics_provider_instance")?;
                            Ok(__StLyricsInstanceOut(out_instance))
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StCreateLyricsProviderInstanceOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &__ST_CREATE_LYRICS_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        return $crate::status_ok();
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "lyrics provider type unsupported")
            })
        }

        extern "C" fn __st_begin_create_output_sink_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_op: *mut stellatune_plugin_api::StCreateOutputSinkInstanceOpRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_begin_create_output_sink_instance", || {
                if out_op.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null out_op");
                }
                let type_id = match unsafe { $crate::ststr_to_str(&type_id_utf8) } {
                    Ok(s) => s,
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                let config_owned = match unsafe { $crate::ststr_to_str(&config_json_utf8) } {
                    Ok(s) => s.to_owned(),
                    Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                };
                $(
                    if type_id == <$sink_ty as $crate::instance::OutputSinkDescriptor>::TYPE_ID {
                        let op = __StCreateOutputOp::spawn(async move {
                            let mut out_instance = stellatune_plugin_api::StOutputSinkInstanceRef {
                                handle: core::ptr::null_mut(),
                                vtable: core::ptr::null(),
                                reserved0: 0,
                                reserved1: 0,
                            };
                            let in_config = $crate::StStr {
                                ptr: config_owned.as_ptr(),
                                len: config_owned.len(),
                            };
                            let status = $sink_mod::create_instance(in_config, &mut out_instance as *mut _);
                            __st_status_to_sdk_result(status, "create_output_sink_instance")?;
                            Ok(__StOutputInstanceOut(out_instance))
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StCreateOutputSinkInstanceOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &__ST_CREATE_OUTPUT_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        return $crate::status_ok();
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "output sink type unsupported")
            })
        }
    };
}
