mod client;
mod config;
mod descriptor;
mod instance;
mod ring;
mod sink;

use core::ffi::c_void;

use client::shutdown_all_sidecars;
use instance::AsioOutputSinkInstance;
use stellatune_plugin_sdk::async_task::{AsyncTaskOp, AsyncTaskTakeError};
use stellatune_plugin_sdk::export_plugin;
use stellatune_plugin_sdk::{
    ST_ERR_INTERNAL, ST_ERR_INVALID_ARG, StAsyncOpState, StOpNotifier, StStatus, StUnitOpRef,
    StUnitOpVTable, status_err_msg, status_ok,
};

type ShutdownUnitOpBox = AsyncTaskOp<()>;

fn shutdown_take_error_to_status(err: AsyncTaskTakeError) -> StStatus {
    match err {
        AsyncTaskTakeError::Pending => {
            status_err_msg(ST_ERR_INTERNAL, "asio shutdown operation not ready")
        },
        AsyncTaskTakeError::Cancelled => {
            status_err_msg(ST_ERR_INTERNAL, "asio shutdown operation cancelled")
        },
        AsyncTaskTakeError::Failed(msg) => {
            if msg.is_empty() {
                status_err_msg(ST_ERR_INTERNAL, "asio shutdown operation failed")
            } else {
                status_err_msg(ST_ERR_INTERNAL, msg)
            }
        },
        AsyncTaskTakeError::AlreadyTaken => status_err_msg(
            ST_ERR_INVALID_ARG,
            "asio shutdown operation result already taken",
        ),
    }
}

extern "C" fn asio_shutdown_op_poll(
    handle: *mut c_void,
    out_state: *mut StAsyncOpState,
) -> StStatus {
    stellatune_plugin_sdk::ffi_guard::guard_status("asio_shutdown_op_poll", || {
        if handle.is_null() || out_state.is_null() {
            return status_err_msg(ST_ERR_INVALID_ARG, "null handle/out_state");
        }
        let op = unsafe { &*(handle as *mut ShutdownUnitOpBox) };
        unsafe {
            *out_state = op.poll();
        }
        status_ok()
    })
}

extern "C" fn asio_shutdown_op_wait(
    handle: *mut c_void,
    timeout_ms: u32,
    out_state: *mut StAsyncOpState,
) -> StStatus {
    stellatune_plugin_sdk::ffi_guard::guard_status("asio_shutdown_op_wait", || {
        if handle.is_null() || out_state.is_null() {
            return status_err_msg(ST_ERR_INVALID_ARG, "null handle/out_state");
        }
        let op = unsafe { &*(handle as *mut ShutdownUnitOpBox) };
        unsafe {
            *out_state = op.wait(timeout_ms);
        }
        status_ok()
    })
}

extern "C" fn asio_shutdown_op_cancel(handle: *mut c_void) -> StStatus {
    stellatune_plugin_sdk::ffi_guard::guard_status("asio_shutdown_op_cancel", || {
        if handle.is_null() {
            return status_err_msg(ST_ERR_INVALID_ARG, "null handle");
        }
        let op = unsafe { &*(handle as *mut ShutdownUnitOpBox) };
        let _ = op.cancel();
        status_ok()
    })
}

extern "C" fn asio_shutdown_op_set_notifier(
    handle: *mut c_void,
    notifier: StOpNotifier,
) -> StStatus {
    stellatune_plugin_sdk::ffi_guard::guard_status("asio_shutdown_op_set_notifier", || {
        if handle.is_null() {
            return status_err_msg(ST_ERR_INVALID_ARG, "null handle");
        }
        let op = unsafe { &*(handle as *mut ShutdownUnitOpBox) };
        op.set_notifier(notifier);
        status_ok()
    })
}

extern "C" fn asio_shutdown_op_finish(handle: *mut c_void) -> StStatus {
    stellatune_plugin_sdk::ffi_guard::guard_status("asio_shutdown_op_finish", || {
        if handle.is_null() {
            return status_err_msg(ST_ERR_INVALID_ARG, "null handle");
        }
        let op = unsafe { &*(handle as *mut ShutdownUnitOpBox) };
        match op.take_result() {
            Ok(()) => status_ok(),
            Err(err) => shutdown_take_error_to_status(err),
        }
    })
}

extern "C" fn asio_shutdown_op_destroy(handle: *mut c_void) {
    stellatune_plugin_sdk::ffi_guard::guard_void("asio_shutdown_op_destroy", || {
        if handle.is_null() {
            return;
        }
        unsafe {
            drop(Box::from_raw(handle as *mut ShutdownUnitOpBox));
        }
    });
}

static ASIO_SHUTDOWN_UNIT_OP_VTABLE: StUnitOpVTable = StUnitOpVTable {
    poll: asio_shutdown_op_poll,
    wait: asio_shutdown_op_wait,
    cancel: asio_shutdown_op_cancel,
    set_notifier: asio_shutdown_op_set_notifier,
    finish: asio_shutdown_op_finish,
    destroy: asio_shutdown_op_destroy,
};

extern "C" fn asio_begin_shutdown(out_op: *mut StUnitOpRef) -> StStatus {
    stellatune_plugin_sdk::ffi_guard::guard_status("asio_begin_shutdown", || {
        if out_op.is_null() {
            return status_err_msg(ST_ERR_INVALID_ARG, "null out_op");
        }

        let op = ShutdownUnitOpBox::spawn(async move { shutdown_all_sidecars() });
        unsafe {
            *out_op = StUnitOpRef {
                handle: Box::into_raw(Box::new(op)) as *mut c_void,
                vtable: &ASIO_SHUTDOWN_UNIT_OP_VTABLE as *const StUnitOpVTable,
                reserved0: 0,
                reserved1: 0,
            };
        }
        status_ok()
    })
}

export_plugin! {
    id: "dev.stellatune.output.asio",
    name: "ASIO Output Sink",
    version: (0, 1, 0),
    decoders: [],
    dsps: [],
    source_catalogs: [],
    lyrics_providers: [],
    output_sinks: [
        asio => AsioOutputSinkInstance,
    ],
    begin_shutdown: asio_begin_shutdown,
}
