use std::path::{Path, PathBuf};

use stellatune_sidecar_support::env::{SidecarLogConfig, build_sidecar_log_config};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidecarTransportKind {
    Stdio,
    NamedPipe,
    UnixSocket,
    LoopbackTcp,
    SharedMemoryRing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidecarTransportOption {
    pub kind: SidecarTransportKind,
    pub priority: u8,
    pub max_frame_bytes: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SidecarLaunchScope {
    Instance,
    Package,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidecarLaunchSpec {
    pub scope: SidecarLaunchScope,
    pub plugin_root: PathBuf,
    pub logging: SidecarLogConfig,
    pub executable: String,
    pub args: Vec<String>,
    pub preferred_control: Vec<SidecarTransportOption>,
    pub preferred_data: Vec<SidecarTransportOption>,
    pub env: Vec<(String, String)>,
}

const HOST_SIDECAR_LOG_LEVEL: &str = "info";

pub(crate) trait SidecarChannelHandle: Send {
    fn transport(&self) -> SidecarTransportKind;
    fn write(&mut self, data: &[u8]) -> Result<u32>;
    fn read(&mut self, max_bytes: u32, timeout_ms: Option<u32>) -> Result<Vec<u8>>;
    fn close(&mut self) {}
}

pub(crate) trait SidecarProcessHandle: Send {
    fn open_control(&mut self) -> Result<Box<dyn SidecarChannelHandle>>;
    fn open_data(
        &mut self,
        role: &str,
        preferred: &[SidecarTransportOption],
    ) -> Result<Box<dyn SidecarChannelHandle>>;
    fn wait_exit(&mut self, timeout_ms: Option<u32>) -> Result<Option<i32>>;
    fn terminate(&mut self, grace_ms: u32) -> Result<()>;
}

pub(crate) trait SidecarHost: Send + Sync {
    fn launch(&self, spec: &SidecarLaunchSpec) -> Result<Box<dyn SidecarProcessHandle>>;
}

pub(crate) fn resolve_sidecar_executable(
    plugin_root: &Path,
    raw_executable: &str,
) -> Result<String> {
    let executable = raw_executable.trim();
    if executable.is_empty() {
        return Err(Error::invalid_input("sidecar executable is empty"));
    }

    let executable_path = Path::new(executable);
    if executable_path.is_absolute() {
        if executable_path.is_file() {
            return Ok(executable.to_string());
        }
        return Err(Error::not_found(
            "sidecar executable",
            executable_path.display().to_string(),
        ));
    }

    if !is_safe_relative_sidecar_path(executable_path) {
        return Err(Error::invalid_input(format!(
            "sidecar executable relative path is unsafe: {}",
            executable
        )));
    }

    let mut candidates = Vec::<PathBuf>::new();
    candidates.push(plugin_root.join(executable_path));
    candidates.push(plugin_root.join("bin").join(executable_path));

    if cfg!(windows)
        && executable_path.extension().is_none()
        && let Some(file_name) = executable_path.file_name().and_then(|name| name.to_str())
    {
        let exe_name = format!("{file_name}.exe");
        if let Some(parent) = executable_path.parent() {
            candidates.push(plugin_root.join(parent).join(&exe_name));
            candidates.push(plugin_root.join("bin").join(parent).join(exe_name));
        } else {
            candidates.push(plugin_root.join(&exe_name));
            candidates.push(plugin_root.join("bin").join(exe_name));
        }
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().to_string());
        }
    }

    Err(Error::not_found(
        "sidecar executable",
        format!(
            "{} (searched under plugin root `{}` and `bin/`)",
            executable,
            plugin_root.display()
        ),
    ))
}

pub(crate) fn resolve_sidecar_logging(
    plugin_root: &Path,
    executable: &str,
) -> Result<SidecarLogConfig> {
    let plugins_dir = plugin_root.parent().ok_or_else(|| {
        Error::invalid_input(format!(
            "plugin root has no parent directory: {}",
            plugin_root.display()
        ))
    })?;
    let plugin_id = plugin_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Error::invalid_input(format!(
                "unable to derive plugin id from plugin root: {}",
                plugin_root.display()
            ))
        })?;
    build_sidecar_log_config(executable, plugins_dir, plugin_id, HOST_SIDECAR_LOG_LEVEL)
        .map_err(|error| Error::operation("sidecar.log-config", error.to_string()))
}

fn is_safe_relative_sidecar_path(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }
    !path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        )
    })
}

pub(super) fn normalize_role_key(role: &str) -> String {
    let mut out = String::new();
    for ch in role.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

pub(super) fn transport_env_suffixes(kind: SidecarTransportKind) -> &'static [&'static str] {
    match kind {
        SidecarTransportKind::Stdio => &["STDIO"],
        SidecarTransportKind::NamedPipe => &["NAMED_PIPE", "PIPE"],
        SidecarTransportKind::UnixSocket => &["UNIX_SOCKET", "UNIX"],
        SidecarTransportKind::LoopbackTcp => &["LOOPBACK_TCP", "TCP"],
        SidecarTransportKind::SharedMemoryRing => &["SHARED_MEMORY_RING", "SHM"],
    }
}

pub(super) fn ordered_kinds(options: &[SidecarTransportOption]) -> Vec<SidecarTransportKind> {
    if options.is_empty() {
        return vec![SidecarTransportKind::Stdio];
    }
    let mut indexed = options
        .iter()
        .enumerate()
        .collect::<Vec<(usize, &SidecarTransportOption)>>();
    indexed.sort_by(|left, right| {
        right
            .1
            .priority
            .cmp(&left.1.priority)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut out = Vec::<SidecarTransportKind>::new();
    for (_, option) in indexed {
        if !out.contains(&option.kind) {
            out.push(option.kind);
        }
    }
    out
}
