//! FFI panic guard utilities.
//!
//! Every `extern "C" fn` exported by the plugin SDK macros must catch panics
//! to avoid undefined behaviour at the FFI boundary.  These helpers centralise
//! that logic so each macro module can simply call `guard_status`, `guard_void`,
//! or `guard_with_default` instead of duplicating `catch_unwind` boilerplate.

use crate::{ST_ERR_INTERNAL, StLogLevel, StStatus, host_log, status_err_msg};

/// Extract a human-readable message from a panic payload.
pub fn panic_message(payload: Box<dyn core::any::Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&'static str>() {
        return (*msg).to_string();
    }
    if let Some(msg) = payload.downcast_ref::<String>() {
        return msg.clone();
    }
    "non-string panic payload".to_string()
}

/// Catch panics in FFI callbacks that return [`StStatus`].
///
/// On panic the error is logged via `host_log` (including a backtrace) and an
/// `ST_ERR_INTERNAL` status is returned to the host.
pub fn guard_status(op: &'static str, f: impl FnOnce() -> StStatus) -> StStatus {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(payload) => {
            let msg = panic_message(payload);
            let bt = std::backtrace::Backtrace::force_capture();
            host_log(
                StLogLevel::Error,
                &format!("panic in ffi `{op}`: {msg}\nbacktrace:\n{bt}"),
            );
            status_err_msg(ST_ERR_INTERNAL, format!("panic in ffi `{op}`: {msg}"))
        }
    }
}

/// Catch panics in FFI callbacks that return nothing (`void`).
///
/// On panic the error is logged but there is no return value to propagate.
pub fn guard_void(op: &'static str, f: impl FnOnce()) {
    if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        let msg = panic_message(payload);
        let bt = std::backtrace::Backtrace::force_capture();
        host_log(
            StLogLevel::Error,
            &format!("panic in ffi `{op}`: {msg}\nbacktrace:\n{bt}"),
        );
    }
}

/// Catch panics in FFI callbacks that return a value with a known safe default.
///
/// On panic the error is logged and `default` is returned.
pub fn guard_with_default<T>(op: &'static str, default: T, f: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(val) => val,
        Err(payload) => {
            let msg = panic_message(payload);
            let bt = std::backtrace::Backtrace::force_capture();
            host_log(
                StLogLevel::Error,
                &format!("panic in ffi `{op}`: {msg}\nbacktrace:\n{bt}"),
            );
            default
        }
    }
}
