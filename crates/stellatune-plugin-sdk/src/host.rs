use core::sync::atomic::{AtomicPtr, Ordering};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use crate::{StHostVTableV1, StLogLevel, StStatus, StStr, ststr_to_str};

static HOST_VTABLE_V1: AtomicPtr<StHostVTableV1> = AtomicPtr::new(core::ptr::null_mut());

#[doc(hidden)]
pub unsafe fn __set_host_vtable_v1(host: *const StHostVTableV1) {
    HOST_VTABLE_V1.store(host as *mut StHostVTableV1, Ordering::Release);
}

/// Log a message to the host, if the host provided a logger.
///
/// This is purely best-effort: if no host logger is present, this is a no-op.
pub fn host_log(level: StLogLevel, msg: &str) {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return;
    }

    // Safety: the host owns the vtable and defines its lifetime.
    let cb = unsafe { (*host).log_utf8 };
    let Some(cb) = cb else {
        return;
    };

    let bytes = msg.as_bytes();
    let st = StStr {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
    };
    let user_data = unsafe { (*host).user_data };
    cb(user_data, level, st);
}

/// Returns runtime root directory assigned by host for this plugin.
pub fn plugin_runtime_root() -> Option<String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return None;
    }
    let cb = unsafe { (*host).get_runtime_root_utf8 }?;
    let user_data = unsafe { (*host).user_data };
    let root = cb(user_data);
    if root.ptr.is_null() || root.len == 0 {
        return None;
    }
    unsafe { ststr_to_str(&root).ok().map(ToOwned::to_owned) }
}

/// Returns runtime root directory assigned by host for this plugin as `PathBuf`.
pub fn plugin_runtime_root_path() -> Option<PathBuf> {
    plugin_runtime_root().map(PathBuf::from)
}

fn host_take_owned_string(host: *const StHostVTableV1, s: StStr) -> String {
    if s.ptr.is_null() || s.len == 0 {
        return String::new();
    }

    let text = unsafe { ststr_to_str(&s) }
        .map(ToOwned::to_owned)
        .unwrap_or_else(|_| String::new());

    let free_cb = unsafe { (*host).free_host_str_utf8 };
    if let Some(free_cb) = free_cb {
        let user_data = unsafe { (*host).user_data };
        free_cb(user_data, s);
    }

    text
}

fn host_status_to_result(
    host: *const StHostVTableV1,
    what: &str,
    status: StStatus,
) -> Result<(), String> {
    if status.code == 0 {
        return Ok(());
    }
    let msg = host_take_owned_string(host, status.message);
    if msg.is_empty() {
        Err(format!("{what} failed (code={})", status.code))
    } else {
        Err(format!("{what} failed (code={}): {msg}", status.code))
    }
}

/// Emit runtime event JSON to host (plugin -> host).
pub fn host_emit_event_json(event_json: &str) -> Result<(), String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return Err("host vtable unavailable".to_string());
    }
    let cb = unsafe { (*host).emit_event_json_utf8 }
        .ok_or_else(|| "host callback `emit_event_json_utf8` unavailable".to_string())?;
    let user_data = unsafe { (*host).user_data };
    let in_json = StStr {
        ptr: event_json.as_ptr(),
        len: event_json.len(),
    };
    let status = cb(user_data, in_json);
    host_status_to_result(host, "emit_event_json_utf8", status)
}

/// Poll one host event JSON (host/flutter -> plugin).
pub fn host_poll_event_json() -> Result<Option<String>, String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return Err("host vtable unavailable".to_string());
    }
    let cb = unsafe { (*host).poll_host_event_json_utf8 }
        .ok_or_else(|| "host callback `poll_host_event_json_utf8` unavailable".to_string())?;
    let user_data = unsafe { (*host).user_data };
    let mut out = StStr::empty();
    let status = cb(user_data, &mut out as *mut StStr);
    host_status_to_result(host, "poll_host_event_json_utf8", status)?;
    if out.ptr.is_null() || out.len == 0 {
        return Ok(None);
    }
    Ok(Some(host_take_owned_string(host, out)))
}

/// Send control request JSON to host and receive immediate response JSON.
pub fn host_send_control_json(request_json: &str) -> Result<String, String> {
    let host = HOST_VTABLE_V1.load(Ordering::Acquire);
    if host.is_null() {
        return Err("host vtable unavailable".to_string());
    }
    let cb = unsafe { (*host).send_control_json_utf8 }
        .ok_or_else(|| "host callback `send_control_json_utf8` unavailable".to_string())?;
    let user_data = unsafe { (*host).user_data };
    let in_json = StStr {
        ptr: request_json.as_ptr(),
        len: request_json.len(),
    };
    let mut out = StStr::empty();
    let status = cb(user_data, in_json, &mut out as *mut StStr);
    host_status_to_result(host, "send_control_json_utf8", status)?;
    Ok(host_take_owned_string(host, out))
}

/// Resolves a path relative to plugin runtime root.
pub fn resolve_runtime_path(relative: impl AsRef<Path>) -> Option<PathBuf> {
    let root = plugin_runtime_root_path()?;
    let rel = relative.as_ref();
    if rel.as_os_str().is_empty() {
        return Some(root);
    }
    if rel.is_absolute() {
        return Some(rel.to_path_buf());
    }
    Some(root.join(rel))
}

/// Build a command to launch a sidecar program under plugin runtime root.
///
/// The current working directory is set to runtime root.
pub fn sidecar_command(relative_program: impl AsRef<Path>) -> io::Result<Command> {
    let root = plugin_runtime_root_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "plugin runtime root is unavailable",
        )
    })?;
    let program = root.join(relative_program.as_ref());
    let mut cmd = Command::new(program);
    cmd.current_dir(root);
    Ok(cmd)
}

/// Spawn a sidecar program under plugin runtime root.
pub fn spawn_sidecar<I, S>(relative_program: impl AsRef<Path>, args: I) -> io::Result<Child>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = sidecar_command(relative_program)?;
    cmd.args(args);
    cmd.spawn()
}
