#[doc(hidden)]
#[macro_export]
macro_rules! __st_export_decoder_dispatch {
    ($( $dec_mod:ident => $dec_ty:ty ),* $(,)?) => {
        struct __StDecoderInstanceOut(stellatune_plugin_api::StDecoderInstanceRef);
        unsafe impl Send for __StDecoderInstanceOut {}
        type __StCreateDecoderOp = $crate::async_task::AsyncTaskOp<__StDecoderInstanceOut>;

        fn __st_decoder_take_error_to_status(
            err: $crate::async_task::AsyncTaskTakeError,
        ) -> $crate::StStatus {
            match err {
                $crate::async_task::AsyncTaskTakeError::Pending => {
                    $crate::status_err_msg($crate::ST_ERR_INTERNAL, "create_decoder_instance operation not ready")
                }
                $crate::async_task::AsyncTaskTakeError::Cancelled => {
                    $crate::status_err_msg($crate::ST_ERR_INTERNAL, "create_decoder_instance operation cancelled")
                }
                $crate::async_task::AsyncTaskTakeError::Failed(msg) => {
                    if msg.is_empty() {
                        $crate::status_err_msg($crate::ST_ERR_INTERNAL, "create_decoder_instance operation failed")
                    } else {
                        $crate::status_err_msg($crate::ST_ERR_INTERNAL, msg)
                    }
                }
                $crate::async_task::AsyncTaskTakeError::AlreadyTaken => {
                    $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "create_decoder_instance result already taken")
                }
            }
        }

        fn __st_decoder_status_to_sdk_result(status: $crate::StStatus) -> $crate::SdkResult<()> {
            if status.code == 0 {
                return Ok(());
            }
            let message = unsafe { $crate::ststr_to_str(&status.message) }
                .unwrap_or("operation failed")
                .to_string();
            if !status.message.ptr.is_null() && status.message.len > 0 {
                $crate::plugin_free(status.message.ptr as *mut core::ffi::c_void, status.message.len, 1);
            }
            Err($crate::SdkError::msg(format!("create_decoder_instance: {message}")))
        }

        extern "C" fn __st_decoder_create_op_poll(
            handle: *mut core::ffi::c_void,
            out_state: *mut stellatune_plugin_api::StAsyncOpState,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_decoder_create_op_poll", || {
                if handle.is_null() || out_state.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_state");
                }
                let op = unsafe { &*(handle as *mut __StCreateDecoderOp) };
                unsafe { *out_state = op.poll(); }
                $crate::status_ok()
            })
        }

        extern "C" fn __st_decoder_create_op_wait(
            handle: *mut core::ffi::c_void,
            timeout_ms: u32,
            out_state: *mut stellatune_plugin_api::StAsyncOpState,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_decoder_create_op_wait", || {
                if handle.is_null() || out_state.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_state");
                }
                let op = unsafe { &*(handle as *mut __StCreateDecoderOp) };
                unsafe { *out_state = op.wait(timeout_ms); }
                $crate::status_ok()
            })
        }

        extern "C" fn __st_decoder_create_op_cancel(
            handle: *mut core::ffi::c_void,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_decoder_create_op_cancel", || {
                if handle.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                }
                let op = unsafe { &*(handle as *mut __StCreateDecoderOp) };
                let _ = op.cancel();
                $crate::status_ok()
            })
        }

        extern "C" fn __st_decoder_create_op_set_notifier(
            handle: *mut core::ffi::c_void,
            notifier: stellatune_plugin_api::StOpNotifier,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_decoder_create_op_set_notifier", || {
                if handle.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                }
                let op = unsafe { &*(handle as *mut __StCreateDecoderOp) };
                op.set_notifier(notifier);
                $crate::status_ok()
            })
        }

        extern "C" fn __st_decoder_create_op_take_instance(
            handle: *mut core::ffi::c_void,
            out_instance: *mut stellatune_plugin_api::StDecoderInstanceRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_decoder_create_op_take_instance", || {
                if handle.is_null() || out_instance.is_null() {
                    return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_instance");
                }
                let op = unsafe { &*(handle as *mut __StCreateDecoderOp) };
                match op.take_result() {
                    Ok(instance) => {
                        unsafe { *out_instance = instance.0; }
                        $crate::status_ok()
                    }
                    Err(err) => __st_decoder_take_error_to_status(err),
                }
            })
        }

        extern "C" fn __st_decoder_create_op_destroy(handle: *mut core::ffi::c_void) {
            $crate::ffi_guard::guard_void("__st_decoder_create_op_destroy", || {
                if handle.is_null() {
                    return;
                }
                unsafe { drop(Box::from_raw(handle as *mut __StCreateDecoderOp)); }
            });
        }

        static __ST_CREATE_DECODER_OP_VTABLE:
            stellatune_plugin_api::StCreateDecoderInstanceOpVTable =
            stellatune_plugin_api::StCreateDecoderInstanceOpVTable {
                poll: __st_decoder_create_op_poll,
                wait: __st_decoder_create_op_wait,
                cancel: __st_decoder_create_op_cancel,
                set_notifier: __st_decoder_create_op_set_notifier,
                take_instance: __st_decoder_create_op_take_instance,
                destroy: __st_decoder_create_op_destroy,
            };

        extern "C" fn __st_decoder_ext_score_count(type_id_utf8: $crate::StStr) -> usize {
            $crate::ffi_guard::guard_with_default("__st_decoder_ext_score_count", 0, || {
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
            })
        }

        extern "C" fn __st_decoder_ext_score_get(
            type_id_utf8: $crate::StStr,
            index: usize,
        ) -> *const stellatune_plugin_api::StDecoderExtScore {
            $crate::ffi_guard::guard_with_default("__st_decoder_ext_score_get", core::ptr::null(), || {
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
            })
        }

        extern "C" fn __st_begin_create_decoder_instance(
            type_id_utf8: $crate::StStr,
            config_json_utf8: $crate::StStr,
            out_op: *mut stellatune_plugin_api::StCreateDecoderInstanceOpRef,
        ) -> $crate::StStatus {
            $crate::ffi_guard::guard_status("__st_begin_create_decoder_instance", || {
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
                    if type_id == <$dec_ty as $crate::instance::DecoderDescriptor>::TYPE_ID {
                        let op = __StCreateDecoderOp::spawn(async move {
                            let mut out_instance = stellatune_plugin_api::StDecoderInstanceRef {
                                handle: core::ptr::null_mut(),
                                vtable: core::ptr::null(),
                                reserved0: 0,
                                reserved1: 0,
                            };
                            let in_config = $crate::StStr {
                                ptr: config_owned.as_ptr(),
                                len: config_owned.len(),
                            };
                            let status = $dec_mod::create_instance(in_config, &mut out_instance as *mut _);
                            __st_decoder_status_to_sdk_result(status)?;
                            Ok(__StDecoderInstanceOut(out_instance))
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StCreateDecoderInstanceOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &__ST_CREATE_DECODER_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        return $crate::status_ok();
                    }
                )*
                $crate::status_err_msg($crate::ST_ERR_UNSUPPORTED, "decoder type unsupported")
            })
        }
    };
}
