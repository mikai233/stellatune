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
                type ListItemsOpBox = $crate::async_task::AsyncTaskOp<String>;
                type OpenStreamOpBox =
                    $crate::async_task::AsyncTaskOp<$crate::instance::SourceOpenResult<StreamImpl>>;
                type UnitOpBox = $crate::async_task::AsyncTaskOp<()>;
                type JsonOpBox = $crate::async_task::AsyncTaskOp<Option<String>>;
                type PlanOpBox = $crate::async_task::AsyncTaskOp<$crate::update::UpdatePlan>;

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

                fn take_error_to_status(op_name: &str, err: $crate::async_task::AsyncTaskTakeError) -> $crate::StStatus {
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

                macro_rules! define_common_op_fns {
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
                                    return $crate::status_err_msg(
                                        $crate::ST_ERR_INVALID_ARG,
                                        "null handle/out_state",
                                    );
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
                                    return $crate::status_err_msg(
                                        $crate::ST_ERR_INVALID_ARG,
                                        "null handle/out_state",
                                    );
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

                define_common_op_fns!(
                    list_items_op_poll,
                    list_items_op_wait,
                    list_items_op_cancel,
                    list_items_op_set_notifier,
                    list_items_op_destroy,
                    ListItemsOpBox,
                    "source_list_items_op"
                );

                define_common_op_fns!(
                    open_stream_op_poll,
                    open_stream_op_wait,
                    open_stream_op_cancel,
                    open_stream_op_set_notifier,
                    open_stream_op_destroy,
                    OpenStreamOpBox,
                    "source_open_stream_op"
                );

                define_common_op_fns!(
                    unit_op_poll,
                    unit_op_wait,
                    unit_op_cancel,
                    unit_op_set_notifier,
                    unit_op_destroy,
                    UnitOpBox,
                    "source_unit_op"
                );

                define_common_op_fns!(
                    json_op_poll,
                    json_op_wait,
                    json_op_cancel,
                    json_op_set_notifier,
                    json_op_destroy,
                    JsonOpBox,
                    "source_json_op"
                );

                define_common_op_fns!(
                    plan_op_poll,
                    plan_op_wait,
                    plan_op_cancel,
                    plan_op_set_notifier,
                    plan_op_destroy,
                    PlanOpBox,
                    "source_plan_op"
                );

                extern "C" fn list_items_op_take_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("list_items_op_take_json_utf8", || {
                        if handle.is_null() || out_json_utf8.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                        }
                        let op = unsafe { &*(handle as *mut ListItemsOpBox) };
                        match op.take_result() {
                            Ok(json) => {
                                unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                                $crate::status_ok()
                            }
                            Err(err) => take_error_to_status("list_items", err),
                        }
                    })
                }

                extern "C" fn open_stream_op_take_stream(
                    handle: *mut core::ffi::c_void,
                    out_io_vtable: *mut *const $crate::StIoVTable,
                    out_io_handle: *mut *mut core::ffi::c_void,
                    out_track_meta_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("open_stream_op_take_stream", || {
                        if handle.is_null() || out_io_vtable.is_null() || out_io_handle.is_null() || out_track_meta_json_utf8.is_null() {
                            return $crate::status_err_msg(
                                $crate::ST_ERR_INVALID_ARG,
                                "null handle/out_io_vtable/out_io_handle/out_track_meta_json_utf8",
                            );
                        }
                        let op = unsafe { &*(handle as *mut OpenStreamOpBox) };
                        match op.take_result() {
                            Ok(opened) => {
                                let $crate::instance::SourceOpenResult { stream, track_meta_json } = opened;
                                let stream_boxed = Box::new($crate::instance::SourceStreamBox { inner: stream });
                                unsafe {
                                    *out_io_vtable = &IO_VTABLE as *const $crate::StIoVTable;
                                    *out_io_handle = Box::into_raw(stream_boxed) as *mut core::ffi::c_void;
                                    *out_track_meta_json_utf8 = track_meta_json
                                        .as_deref()
                                        .map($crate::alloc_utf8_bytes)
                                        .unwrap_or_else($crate::StStr::empty);
                                }
                                $crate::status_ok()
                            }
                            Err(err) => take_error_to_status("open_stream", err),
                        }
                    })
                }

                extern "C" fn unit_op_finish(handle: *mut core::ffi::c_void) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("unit_op_finish", || {
                        if handle.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle");
                        }
                        let op = unsafe { &*(handle as *mut UnitOpBox) };
                        match op.take_result() {
                            Ok(()) => $crate::status_ok(),
                            Err(err) => take_error_to_status("unit", err),
                        }
                    })
                }

                extern "C" fn json_op_take_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_json_utf8: *mut $crate::StStr,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("json_op_take_json_utf8", || {
                        if handle.is_null() || out_json_utf8.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_json_utf8");
                        }
                        let op = unsafe { &*(handle as *mut JsonOpBox) };
                        match op.take_result() {
                            Ok(Some(json)) => {
                                unsafe { *out_json_utf8 = $crate::alloc_utf8_bytes(&json); }
                                $crate::status_ok()
                            }
                            Ok(None) => {
                                unsafe { *out_json_utf8 = $crate::StStr::empty(); }
                                $crate::status_ok()
                            }
                            Err(err) => take_error_to_status("json", err),
                        }
                    })
                }

                extern "C" fn plan_op_take_plan(
                    handle: *mut core::ffi::c_void,
                    out_plan: *mut stellatune_plugin_api::StConfigUpdatePlan,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("plan_op_take_plan", || {
                        if handle.is_null() || out_plan.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_plan");
                        }
                        let op = unsafe { &*(handle as *mut PlanOpBox) };
                        match op.take_result() {
                            Ok(plan) => match unsafe { $crate::update::write_plan_to_ffi(out_plan, plan) } {
                                Ok(()) => $crate::status_ok(),
                                Err(e) => $crate::status_err_msg($crate::ST_ERR_INTERNAL, e),
                            },
                            Err(err) => take_error_to_status("plan", err),
                        }
                    })
                }

                extern "C" fn begin_list_items_json_utf8(
                    handle: *mut core::ffi::c_void,
                    request_json_utf8: $crate::StStr,
                    out_op: *mut stellatune_plugin_api::StSourceListItemsOpRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("begin_list_items_json_utf8", || {
                        if handle.is_null() || out_op.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_op");
                        }
                        let request_json = match unsafe { $crate::ststr_to_str(&request_json_utf8) } {
                            Ok(v) => v.to_owned(),
                            Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                        };
                        let boxed = unsafe { &*(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                        let catalog = boxed.inner.clone();
                        let op = ListItemsOpBox::spawn(async move {
                            let mut guard = catalog.lock().await;
                            <CatalogImpl as $crate::instance::SourceCatalogInstance>::list_items_json(&mut *guard, &request_json).await
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StSourceListItemsOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &LIST_ITEMS_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        $crate::status_ok()
                    })
                }

                extern "C" fn io_read(
                    handle: *mut core::ffi::c_void,
                    out: *mut u8,
                    len: usize,
                    out_read: *mut usize,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("io_read", || {
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
                    })
                }

                extern "C" fn io_seek(
                    handle: *mut core::ffi::c_void,
                    offset: i64,
                    whence: $crate::StSeekWhence,
                    out_pos: *mut u64,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("io_seek", || {
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
                    })
                }

                extern "C" fn io_tell(
                    handle: *mut core::ffi::c_void,
                    out_pos: *mut u64,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("io_tell", || {
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
                    })
                }

                extern "C" fn io_size(
                    handle: *mut core::ffi::c_void,
                    out_size: *mut u64,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("io_size", || {
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
                    })
                }

                extern "C" fn begin_open_stream(
                    handle: *mut core::ffi::c_void,
                    track_json_utf8: $crate::StStr,
                    out_op: *mut stellatune_plugin_api::StSourceOpenStreamOpRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("begin_open_stream", || {
                        if handle.is_null() || out_op.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_op");
                        }
                        let track_json = match unsafe { $crate::ststr_to_str(&track_json_utf8) } {
                            Ok(v) => v.to_owned(),
                            Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                        };
                        let boxed = unsafe { &*(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                        let catalog = boxed.inner.clone();
                        let op = OpenStreamOpBox::spawn(async move {
                            let mut guard = catalog.lock().await;
                            <CatalogImpl as $crate::instance::SourceCatalogInstance>::open_stream_json(&mut *guard, &track_json).await
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StSourceOpenStreamOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &OPEN_STREAM_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        $crate::status_ok()
                    })
                }

                extern "C" fn begin_close_stream(
                    handle: *mut core::ffi::c_void,
                    io_handle: *mut core::ffi::c_void,
                    out_op: *mut stellatune_plugin_api::StUnitOpRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("begin_close_stream", || {
                        if handle.is_null() || io_handle.is_null() || out_op.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/io_handle/out_op");
                        }
                        let boxed = unsafe { &*(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                        let catalog = boxed.inner.clone();
                        let mut stream =
                            unsafe { Box::from_raw(io_handle as *mut $crate::instance::SourceStreamBox<StreamImpl>) };
                        let op = UnitOpBox::spawn(async move {
                            let mut guard = catalog.lock().await;
                            <CatalogImpl as $crate::instance::SourceCatalogInstance>::close_stream(
                                &mut *guard,
                                &mut stream.inner,
                            )
                            .await
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StUnitOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &UNIT_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        $crate::status_ok()
                    })
                }

                extern "C" fn begin_plan_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_op: *mut stellatune_plugin_api::StConfigUpdatePlanOpRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("begin_plan_config_update_json_utf8", || {
                        if handle.is_null() || out_op.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_op");
                        }
                        let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                            Ok(v) => v.to_owned(),
                            Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                        };
                        let boxed = unsafe { &*(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                        let catalog = boxed.inner.clone();
                        let op = PlanOpBox::spawn(async move {
                            let guard = catalog.lock().await;
                            <CatalogImpl as $crate::update::ConfigUpdatable>::plan_config_update_json(
                                &*guard,
                                &new_json,
                            )
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StConfigUpdatePlanOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &PLAN_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        $crate::status_ok()
                    })
                }

                extern "C" fn begin_apply_config_update_json_utf8(
                    handle: *mut core::ffi::c_void,
                    new_config_json_utf8: $crate::StStr,
                    out_op: *mut stellatune_plugin_api::StUnitOpRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("begin_apply_config_update_json_utf8", || {
                        if handle.is_null() || out_op.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_op");
                        }
                        let new_json = match unsafe { $crate::ststr_to_str(&new_config_json_utf8) } {
                            Ok(v) => v.to_owned(),
                            Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                        };
                        let boxed = unsafe { &*(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                        let catalog = boxed.inner.clone();
                        let op = UnitOpBox::spawn(async move {
                            let mut guard = catalog.lock().await;
                            <CatalogImpl as $crate::update::ConfigUpdatable>::apply_config_update_json(
                                &mut *guard,
                                &new_json,
                            )
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StUnitOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &UNIT_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        $crate::status_ok()
                    })
                }

                extern "C" fn begin_export_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    out_op: *mut stellatune_plugin_api::StJsonOpRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("begin_export_state_json_utf8", || {
                        if handle.is_null() || out_op.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_op");
                        }
                        let boxed = unsafe { &*(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                        let catalog = boxed.inner.clone();
                        let op = JsonOpBox::spawn(async move {
                            let guard = catalog.lock().await;
                            <CatalogImpl as $crate::update::ConfigUpdatable>::export_state_json(&*guard)
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StJsonOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &JSON_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        $crate::status_ok()
                    })
                }

                extern "C" fn begin_import_state_json_utf8(
                    handle: *mut core::ffi::c_void,
                    state_json_utf8: $crate::StStr,
                    out_op: *mut stellatune_plugin_api::StUnitOpRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("begin_import_state_json_utf8", || {
                        if handle.is_null() || out_op.is_null() {
                            return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, "null handle/out_op");
                        }
                        let state_json = match unsafe { $crate::ststr_to_str(&state_json_utf8) } {
                            Ok(v) => v.to_owned(),
                            Err(e) => return $crate::status_err_msg($crate::ST_ERR_INVALID_ARG, e),
                        };
                        let boxed = unsafe { &*(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>) };
                        let catalog = boxed.inner.clone();
                        let op = UnitOpBox::spawn(async move {
                            let mut guard = catalog.lock().await;
                            <CatalogImpl as $crate::update::ConfigUpdatable>::import_state_json(
                                &mut *guard,
                                &state_json,
                            )
                        });
                        unsafe {
                            *out_op = stellatune_plugin_api::StUnitOpRef {
                                handle: Box::into_raw(Box::new(op)) as *mut core::ffi::c_void,
                                vtable: &UNIT_OP_VTABLE as *const _,
                                reserved0: 0,
                                reserved1: 0,
                            };
                        }
                        $crate::status_ok()
                    })
                }

                extern "C" fn destroy(handle: *mut core::ffi::c_void) {
                    $crate::ffi_guard::guard_void("source_destroy", || {
                        if handle.is_null() {
                            return;
                        }
                        unsafe { drop(Box::from_raw(handle as *mut $crate::instance::SourceCatalogBox<CatalogImpl>)); }
                    });
                }

                pub static IO_VTABLE: $crate::StIoVTable = $crate::StIoVTable {
                    read: io_read,
                    seek: if <StreamImpl as $crate::instance::SourceStream>::SUPPORTS_SEEK { Some(io_seek) } else { None },
                    tell: if <StreamImpl as $crate::instance::SourceStream>::SUPPORTS_TELL { Some(io_tell) } else { None },
                    size: if <StreamImpl as $crate::instance::SourceStream>::SUPPORTS_SIZE { Some(io_size) } else { None },
                };

                pub static LIST_ITEMS_OP_VTABLE: stellatune_plugin_api::StSourceListItemsOpVTable =
                    stellatune_plugin_api::StSourceListItemsOpVTable {
                        poll: list_items_op_poll,
                        wait: list_items_op_wait,
                        cancel: list_items_op_cancel,
                        set_notifier: list_items_op_set_notifier,
                        take_json_utf8: list_items_op_take_json_utf8,
                        destroy: list_items_op_destroy,
                    };

                pub static OPEN_STREAM_OP_VTABLE: stellatune_plugin_api::StSourceOpenStreamOpVTable =
                    stellatune_plugin_api::StSourceOpenStreamOpVTable {
                        poll: open_stream_op_poll,
                        wait: open_stream_op_wait,
                        cancel: open_stream_op_cancel,
                        set_notifier: open_stream_op_set_notifier,
                        take_stream: open_stream_op_take_stream,
                        destroy: open_stream_op_destroy,
                    };

                pub static UNIT_OP_VTABLE: stellatune_plugin_api::StUnitOpVTable =
                    stellatune_plugin_api::StUnitOpVTable {
                        poll: unit_op_poll,
                        wait: unit_op_wait,
                        cancel: unit_op_cancel,
                        set_notifier: unit_op_set_notifier,
                        finish: unit_op_finish,
                        destroy: unit_op_destroy,
                    };

                pub static JSON_OP_VTABLE: stellatune_plugin_api::StJsonOpVTable =
                    stellatune_plugin_api::StJsonOpVTable {
                        poll: json_op_poll,
                        wait: json_op_wait,
                        cancel: json_op_cancel,
                        set_notifier: json_op_set_notifier,
                        take_json_utf8: json_op_take_json_utf8,
                        destroy: json_op_destroy,
                    };

                pub static PLAN_OP_VTABLE: stellatune_plugin_api::StConfigUpdatePlanOpVTable =
                    stellatune_plugin_api::StConfigUpdatePlanOpVTable {
                        poll: plan_op_poll,
                        wait: plan_op_wait,
                        cancel: plan_op_cancel,
                        set_notifier: plan_op_set_notifier,
                        take_plan: plan_op_take_plan,
                        destroy: plan_op_destroy,
                    };

                pub static VTABLE: stellatune_plugin_api::StSourceCatalogInstanceVTable =
                    stellatune_plugin_api::StSourceCatalogInstanceVTable {
                        begin_list_items_json_utf8,
                        begin_open_stream,
                        begin_close_stream,
                        begin_plan_config_update_json_utf8: Some(begin_plan_config_update_json_utf8),
                        begin_apply_config_update_json_utf8: Some(begin_apply_config_update_json_utf8),
                        begin_export_state_json_utf8: Some(begin_export_state_json_utf8),
                        begin_import_state_json_utf8: Some(begin_import_state_json_utf8),
                        destroy,
                    };

                pub extern "C" fn create_instance(
                    config_json_utf8: $crate::StStr,
                    out_instance: *mut stellatune_plugin_api::StSourceCatalogInstanceRef,
                ) -> $crate::StStatus {
                    $crate::ffi_guard::guard_status("source_create_instance", || {
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
                                let boxed = Box::new($crate::instance::SourceCatalogBox {
                                    inner: std::sync::Arc::new($crate::__private::tokio::sync::Mutex::new(instance)),
                                });
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
                    })
                }
            }
        )*
    };
}
