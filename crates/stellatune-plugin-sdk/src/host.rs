use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, Ordering};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{
    STELLATUNE_PLUGIN_API_VERSION_V1, SdkError, SdkResult, StHostVTableV1, StLogLevel, StStatus,
    StStr, ststr_to_str,
};

static HOST_VTABLE_V1: AtomicPtr<StHostVTableV1> = AtomicPtr::new(core::ptr::null_mut());

#[derive(Clone, Copy)]
pub struct HostContext {
    vtable: *const StHostVTableV1,
}

unsafe impl Send for HostContext {}
unsafe impl Sync for HostContext {}

impl HostContext {
    pub fn current() -> SdkResult<Self> {
        let vtable = HOST_VTABLE_V1.load(Ordering::Acquire);
        if vtable.is_null() {
            return Err(SdkError::HostUnavailable);
        }
        let api_version = unsafe { (*vtable).api_version };
        if api_version != STELLATUNE_PLUGIN_API_VERSION_V1 {
            return Err(SdkError::HostApiVersionMismatch {
                expected: STELLATUNE_PLUGIN_API_VERSION_V1,
                actual: api_version,
            });
        }
        Ok(Self { vtable })
    }

    #[inline]
    fn user_data(self) -> *mut c_void {
        unsafe { (*self.vtable).user_data }
    }

    fn take_owned_string(self, s: StStr) -> String {
        if s.ptr.is_null() || s.len == 0 {
            return String::new();
        }

        let text = unsafe { ststr_to_str(&s) }
            .map(ToOwned::to_owned)
            .unwrap_or_else(|_| String::new());

        let free_cb = unsafe { (*self.vtable).free_host_str_utf8 };
        if let Some(free_cb) = free_cb {
            free_cb(self.user_data(), s);
        }

        text
    }

    fn status_to_result(self, what: &'static str, status: StStatus) -> SdkResult<()> {
        if status.code == 0 {
            return Ok(());
        }
        let msg = self.take_owned_string(status.message);
        let message = if msg.is_empty() { None } else { Some(msg) };
        Err(SdkError::HostOperationFailed {
            operation: what,
            code: status.code,
            message,
        })
    }

    pub fn log(self, level: StLogLevel, msg: &str) {
        let cb = unsafe { (*self.vtable).log_utf8 };
        let Some(cb) = cb else {
            return;
        };
        let bytes = msg.as_bytes();
        let st = StStr {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
        };
        cb(self.user_data(), level, st);
    }

    pub fn plugin_runtime_root(self) -> Option<String> {
        let cb = unsafe { (*self.vtable).get_runtime_root_utf8 }?;
        let root = cb(self.user_data());
        if root.ptr.is_null() || root.len == 0 {
            return None;
        }
        unsafe { ststr_to_str(&root).ok().map(ToOwned::to_owned) }
    }

    pub fn plugin_runtime_root_path(self) -> Option<PathBuf> {
        self.plugin_runtime_root().map(PathBuf::from)
    }

    pub(crate) fn emit_event_json(self, event_json: &str) -> SdkResult<()> {
        let cb = unsafe { (*self.vtable).emit_event_json_utf8 }
            .ok_or(SdkError::HostCallbackUnavailable("emit_event_json_utf8"))?;
        let in_json = StStr {
            ptr: event_json.as_ptr(),
            len: event_json.len(),
        };
        let status = cb(self.user_data(), in_json);
        self.status_to_result("emit_event_json_utf8", status)
    }

    pub(crate) fn poll_event_json(self) -> SdkResult<Option<String>> {
        let cb = unsafe { (*self.vtable).poll_host_event_json_utf8 }.ok_or(
            SdkError::HostCallbackUnavailable("poll_host_event_json_utf8"),
        )?;
        let mut out = StStr::empty();
        let status = cb(self.user_data(), &mut out as *mut StStr);
        self.status_to_result("poll_host_event_json_utf8", status)?;
        if out.ptr.is_null() || out.len == 0 {
            return Ok(None);
        }
        Ok(Some(self.take_owned_string(out)))
    }

    pub(crate) fn send_control_json(self, request_json: &str) -> SdkResult<String> {
        let cb = unsafe { (*self.vtable).send_control_json_utf8 }
            .ok_or(SdkError::HostCallbackUnavailable("send_control_json_utf8"))?;
        let in_json = StStr {
            ptr: request_json.as_ptr(),
            len: request_json.len(),
        };
        let mut out = StStr::empty();
        let status = cb(self.user_data(), in_json, &mut out as *mut StStr);
        self.status_to_result("send_control_json_utf8", status)?;
        Ok(self.take_owned_string(out))
    }

    pub fn resolve_runtime_path(self, relative: impl AsRef<Path>) -> Option<PathBuf> {
        let root = self.plugin_runtime_root_path()?;
        let rel = relative.as_ref();
        if rel.as_os_str().is_empty() {
            return Some(root);
        }
        if rel.is_absolute() {
            return Some(rel.to_path_buf());
        }
        Some(root.join(rel))
    }

    pub fn sidecar_command(self, relative_program: impl AsRef<Path>) -> io::Result<Command> {
        let root = self.plugin_runtime_root_path().ok_or_else(|| {
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

    pub fn spawn_sidecar<I, S>(
        self,
        relative_program: impl AsRef<Path>,
        args: I,
    ) -> io::Result<Child>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let mut cmd = self.sidecar_command(relative_program)?;
        cmd.args(args);
        cmd.spawn()
    }
}

#[doc(hidden)]
pub unsafe fn __set_host_vtable_v1(host: *const StHostVTableV1) {
    HOST_VTABLE_V1.store(host as *mut StHostVTableV1, Ordering::Release);
}

pub fn host_context() -> SdkResult<HostContext> {
    HostContext::current()
}

/// Log a message to the host, if the host provided a logger.
///
/// This is purely best-effort: if no host logger is present, this is a no-op.
pub fn host_log(level: StLogLevel, msg: &str) {
    if let Ok(host) = HostContext::current() {
        host.log(level, msg);
    }
}

/// Returns runtime root directory assigned by host for this plugin.
pub fn plugin_runtime_root() -> Option<String> {
    HostContext::current()
        .ok()
        .and_then(HostContext::plugin_runtime_root)
}

/// Returns runtime root directory assigned by host for this plugin as `PathBuf`.
pub fn plugin_runtime_root_path() -> Option<PathBuf> {
    HostContext::current()
        .ok()
        .and_then(HostContext::plugin_runtime_root_path)
}

pub(crate) fn host_emit_event_json(event_json: &str) -> SdkResult<()> {
    HostContext::current()?.emit_event_json(event_json)
}

pub(crate) fn host_poll_event_json() -> SdkResult<Option<String>> {
    HostContext::current()?.poll_event_json()
}

pub(crate) fn host_send_control_json(request_json: &str) -> SdkResult<String> {
    HostContext::current()?.send_control_json(request_json)
}

/// Emit typed runtime event to host (plugin -> host).
pub fn host_emit_event<T: Serialize>(event: &T) -> SdkResult<()> {
    let raw = serde_json::to_string(event).map_err(SdkError::from)?;
    host_emit_event_json(&raw)
}

/// Poll one typed host event (host/flutter -> plugin).
pub fn host_poll_event_typed<T: DeserializeOwned>() -> SdkResult<Option<T>> {
    let Some(raw) = host_poll_event_json()? else {
        return Ok(None);
    };
    let parsed = serde_json::from_str::<T>(&raw).map_err(SdkError::from)?;
    Ok(Some(parsed))
}

/// Resolves a path relative to plugin runtime root.
pub fn resolve_runtime_path(relative: impl AsRef<Path>) -> Option<PathBuf> {
    HostContext::current().ok()?.resolve_runtime_path(relative)
}

/// Build a command to launch a sidecar program under plugin runtime root.
///
/// The current working directory is set to runtime root.
pub fn sidecar_command(relative_program: impl AsRef<Path>) -> io::Result<Command> {
    HostContext::current()
        .map_err(io::Error::other)?
        .sidecar_command(relative_program)
}

/// Spawn a sidecar program under plugin runtime root.
pub fn spawn_sidecar<I, S>(relative_program: impl AsRef<Path>, args: I) -> io::Result<Child>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    HostContext::current()
        .map_err(io::Error::other)?
        .spawn_sidecar(relative_program, args)
}
