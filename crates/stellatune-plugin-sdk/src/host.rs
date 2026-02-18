use core::ffi::c_void;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use stellatune_plugin_api::{STELLATUNE_PLUGIN_API_VERSION, StHostVTable};

use crate::{SdkError, SdkResult, StLogLevel, StStr, ststr_to_str};

#[derive(Clone, Copy)]
pub struct HostContext {
    vtable: *const StHostVTable,
}

unsafe impl Send for HostContext {}

impl HostContext {
    pub fn current() -> SdkResult<Self> {
        let Some(vtable) = crate::export::host_vtable_raw() else {
            return Err(SdkError::HostUnavailable);
        };
        if vtable.is_null() {
            return Err(SdkError::HostUnavailable);
        }
        let api_version = unsafe { (*vtable).api_version };
        if api_version != STELLATUNE_PLUGIN_API_VERSION {
            return Err(SdkError::HostApiVersionMismatch {
                expected: STELLATUNE_PLUGIN_API_VERSION,
                actual: api_version,
            });
        }
        Ok(Self { vtable })
    }

    #[inline]
    fn user_data(self) -> *mut c_void {
        unsafe { (*self.vtable).user_data }
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

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            if !cfg!(debug_assertions) {
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
        }

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
