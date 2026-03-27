use std::collections::BTreeMap;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
#[cfg(windows)]
use std::mem::size_of;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
#[cfg(windows)]
use windows::core::PCWSTR;

use crate::error::{Error, Result};

use super::channel::{ChannelHandle, ChannelIo, ChildIo};
#[cfg(unix)]
use super::shm::unlink_named_semaphore;
use super::shm::{NamedEventHandle, SharedMemoryChannelIo, prepare_shared_memory_env};
use super::types::{
    SidecarChannelHandle, SidecarHost, SidecarLaunchSpec, SidecarProcessHandle,
    SidecarTransportKind, SidecarTransportOption, normalize_role_key, ordered_kinds,
    transport_env_suffixes,
};

pub(crate) fn default_sidecar_host() -> Arc<dyn SidecarHost> {
    Arc::new(ProcessSidecarHost)
}

struct ProcessSidecarHost;

impl SidecarHost for ProcessSidecarHost {
    fn launch(&self, spec: &SidecarLaunchSpec) -> Result<Box<dyn SidecarProcessHandle>> {
        let executable = spec.executable.trim();
        if executable.is_empty() {
            return Err(Error::invalid_input("sidecar executable is empty"));
        }

        let mut env = spec.env.clone();
        let mut env_map = build_env_map(&env);
        let mut created_ring_paths = Vec::<PathBuf>::new();
        #[cfg(unix)]
        let mut created_semaphore_names = Vec::<String>::new();
        #[cfg(windows)]
        let mut created_event_handles = Vec::<NamedEventHandle>::new();
        prepare_shared_memory_env(
            &spec.preferred_control,
            "STELLATUNE_SIDECAR_CONTROL_SHARED_MEMORY_RING",
            "STELLATUNE_SIDECAR_CONTROL_SHM",
            &mut env,
            &mut env_map,
            &mut created_ring_paths,
            #[cfg(unix)]
            &mut created_semaphore_names,
            #[cfg(windows)]
            &mut created_event_handles,
        )?;
        prepare_shared_memory_env(
            &spec.preferred_data,
            "STELLATUNE_SIDECAR_DATA_SHARED_MEMORY_RING",
            "STELLATUNE_SIDECAR_DATA_SHM",
            &mut env,
            &mut env_map,
            &mut created_ring_paths,
            #[cfg(unix)]
            &mut created_semaphore_names,
            #[cfg(windows)]
            &mut created_event_handles,
        )?;

        let mut command = Command::new(executable);
        command.args(spec.args.iter().map(String::as_str));
        command.envs(
            env.iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        );
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::null());
        #[cfg(unix)]
        configure_unix_sidecar_command(&mut command);

        let mut child = command
            .spawn()
            .map_err(|error| Error::operation("sidecar.launch", error.to_string()))?;
        #[cfg(windows)]
        let kill_on_close_job = match KillOnCloseJob::attach(&child) {
            Ok(job) => Some(job),
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "failed to attach sidecar process to kill-on-close job object"
                );
                None
            },
        };
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::operation("sidecar.launch", "missing stdin pipe"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::operation("sidecar.launch", "missing stdout pipe"))?;
        #[cfg(unix)]
        let unix_process_group = Some(UnixProcessGroup::new(child.id() as libc::pid_t));

        Ok(Box::new(ProcessHandle {
            inner: Arc::new(Mutex::new(ChildIo {
                child,
                stdin: Some(stdin),
                stdout,
            })),
            control_preferred: spec.preferred_control.clone(),
            data_preferred: spec.preferred_data.clone(),
            env_map,
            created_ring_paths,
            #[cfg(unix)]
            created_semaphore_names,
            #[cfg(unix)]
            unix_process_group,
            #[cfg(windows)]
            _created_event_handles: created_event_handles,
            #[cfg(windows)]
            _kill_on_close_job: kill_on_close_job,
        }))
    }
}

pub(super) struct ProcessHandle {
    pub(super) inner: Arc<Mutex<ChildIo>>,
    control_preferred: Vec<SidecarTransportOption>,
    data_preferred: Vec<SidecarTransportOption>,
    env_map: BTreeMap<String, String>,
    created_ring_paths: Vec<PathBuf>,
    #[cfg(unix)]
    created_semaphore_names: Vec<String>,
    #[cfg(unix)]
    unix_process_group: Option<UnixProcessGroup>,
    #[cfg(windows)]
    _created_event_handles: Vec<NamedEventHandle>,
    #[cfg(windows)]
    _kill_on_close_job: Option<KillOnCloseJob>,
}

#[cfg(unix)]
struct UnixProcessGroup {
    pgid: libc::pid_t,
}

#[cfg(windows)]
struct KillOnCloseJob {
    handle: HANDLE,
}

#[cfg(windows)]
impl KillOnCloseJob {
    fn attach(child: &Child) -> Result<Self> {
        let job = unsafe {
            CreateJobObjectW(None, PCWSTR::null())
                .map_err(|error| Error::operation("sidecar.launch.job-object", error.to_string()))?
        };

        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let limits_ptr = (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast();
        let limits_size = size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        if let Err(error) = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                limits_ptr,
                limits_size,
            )
        } {
            unsafe {
                let _ = CloseHandle(job);
            }
            return Err(Error::operation(
                "sidecar.launch.job-object",
                error.to_string(),
            ));
        }

        let process_handle = HANDLE(child.as_raw_handle());
        if let Err(error) = unsafe { AssignProcessToJobObject(job, process_handle) } {
            unsafe {
                let _ = CloseHandle(job);
            }
            return Err(Error::operation(
                "sidecar.launch.job-object",
                error.to_string(),
            ));
        }

        Ok(Self { handle: job })
    }
}

#[cfg(unix)]
impl UnixProcessGroup {
    fn new(pgid: libc::pid_t) -> Self {
        Self { pgid }
    }

    fn signal(&self, signal: libc::c_int) -> Result<()> {
        let result = unsafe { libc::kill(-self.pgid, signal) };
        if result == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(code) if code == libc::ESRCH) {
            return Ok(());
        }
        Err(Error::operation(
            "sidecar.terminate.process-group",
            error.to_string(),
        ))
    }
}

#[cfg(unix)]
fn configure_unix_sidecar_command(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }

            #[cfg(target_os = "linux")]
            {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() == 1 {
                    return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
                }
            }

            Ok(())
        });
    }
}

#[cfg(windows)]
unsafe impl Send for KillOnCloseJob {}

#[cfg(windows)]
impl Drop for KillOnCloseJob {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

impl ProcessHandle {
    fn channel_from_options(
        &mut self,
        role: Option<&str>,
        options: &[SidecarTransportOption],
    ) -> Result<Box<dyn SidecarChannelHandle>> {
        let mut kinds = ordered_kinds(options);
        if !kinds.contains(&SidecarTransportKind::Stdio) {
            kinds.push(SidecarTransportKind::Stdio);
        }

        let mut errors = Vec::<String>::new();
        for kind in kinds {
            match self.try_open_channel(role, kind) {
                Ok(channel) => return Ok(channel),
                Err(error) => errors.push(format!("{kind:?}: {error}")),
            }
        }
        Err(Error::aggregate("sidecar.open-channel", errors))
    }

    fn try_open_channel(
        &mut self,
        role: Option<&str>,
        kind: SidecarTransportKind,
    ) -> Result<Box<dyn SidecarChannelHandle>> {
        match kind {
            SidecarTransportKind::Stdio => Ok(Box::new(ChannelHandle::stdio(self.inner.clone()))),
            SidecarTransportKind::LoopbackTcp => {
                let endpoint = self.resolve_endpoint(role, kind).ok_or_else(|| {
                    Error::unsupported(format!(
                        "missing endpoint env for loopback tcp role={}",
                        role.unwrap_or("control")
                    ))
                })?;
                let stream = std::net::TcpStream::connect(endpoint.as_str()).map_err(|error| {
                    Error::operation("sidecar.open-loopback-tcp", error.to_string())
                })?;
                Ok(Box::new(ChannelHandle::transport(
                    SidecarTransportKind::LoopbackTcp,
                    ChannelIo::Tcp(stream),
                )))
            },
            SidecarTransportKind::NamedPipe => {
                let endpoint = self.resolve_endpoint(role, kind).ok_or_else(|| {
                    Error::unsupported(format!(
                        "missing endpoint env for named pipe role={}",
                        role.unwrap_or("control")
                    ))
                })?;
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(endpoint.as_str())
                    .map_err(|error| {
                        Error::operation("sidecar.open-named-pipe", error.to_string())
                    })?;
                Ok(Box::new(ChannelHandle::transport(
                    SidecarTransportKind::NamedPipe,
                    ChannelIo::File(file),
                )))
            },
            SidecarTransportKind::UnixSocket => {
                let endpoint = self.resolve_endpoint(role, kind).ok_or_else(|| {
                    Error::unsupported(format!(
                        "missing endpoint env for unix socket role={}",
                        role.unwrap_or("control")
                    ))
                })?;
                #[cfg(unix)]
                {
                    let stream = std::os::unix::net::UnixStream::connect(endpoint.as_str())
                        .map_err(|error| {
                            Error::operation("sidecar.open-unix-socket", error.to_string())
                        })?;
                    Ok(Box::new(ChannelHandle::transport(
                        SidecarTransportKind::UnixSocket,
                        ChannelIo::Unix(stream),
                    )))
                }
                #[cfg(not(unix))]
                {
                    let _ = endpoint;
                    Err(Error::unsupported(
                        "unix-socket transport is not available on this platform",
                    ))
                }
            },
            SidecarTransportKind::SharedMemoryRing => {
                let endpoint = self.resolve_endpoint(role, kind).ok_or_else(|| {
                    Error::unsupported(format!(
                        "missing endpoint env for shared-memory-ring role={}",
                        role.unwrap_or("control")
                    ))
                })?;
                let shared = SharedMemoryChannelIo::open(endpoint.as_str())?;
                Ok(Box::new(ChannelHandle::transport(
                    SidecarTransportKind::SharedMemoryRing,
                    ChannelIo::SharedMemory(shared),
                )))
            },
        }
    }

    fn resolve_endpoint(&self, role: Option<&str>, kind: SidecarTransportKind) -> Option<String> {
        let mut keys = Vec::<String>::new();
        let suffixes = transport_env_suffixes(kind);
        match role {
            Some(role) => {
                let role_key = normalize_role_key(role);
                if !role_key.is_empty() {
                    for suffix in suffixes {
                        keys.push(format!("STELLATUNE_SIDECAR_DATA_{}_{}", role_key, suffix));
                    }
                }
                for suffix in suffixes {
                    keys.push(format!("STELLATUNE_SIDECAR_DATA_{suffix}"));
                }
            },
            None => {
                for suffix in suffixes {
                    keys.push(format!("STELLATUNE_SIDECAR_CONTROL_{suffix}"));
                }
            },
        }

        for key in keys {
            if let Some(value) = self.env_map.get(key.as_str()) {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    fn merged_data_preferred(
        &self,
        preferred: &[SidecarTransportOption],
    ) -> Vec<SidecarTransportOption> {
        if preferred.is_empty() {
            return self.data_preferred.clone();
        }
        let mut merged = preferred.to_vec();
        for option in &self.data_preferred {
            if merged.iter().all(|item| item.kind != option.kind) {
                merged.push(option.clone());
            }
        }
        merged
    }
}

impl SidecarProcessHandle for ProcessHandle {
    fn open_control(&mut self) -> Result<Box<dyn SidecarChannelHandle>> {
        let preferred = self.control_preferred.clone();
        self.channel_from_options(None, &preferred)
    }

    fn open_data(
        &mut self,
        role: &str,
        preferred: &[SidecarTransportOption],
    ) -> Result<Box<dyn SidecarChannelHandle>> {
        let merged = self.merged_data_preferred(preferred);
        self.channel_from_options(Some(role.trim()), &merged)
    }

    fn wait_exit(&mut self, timeout_ms: Option<u32>) -> Result<Option<i32>> {
        let mut inner = self.inner.lock();
        match timeout_ms {
            None => {
                let status = inner
                    .child
                    .wait()
                    .map_err(|error| Error::operation("sidecar.wait-exit", error.to_string()))?;
                Ok(status.code())
            },
            Some(timeout) => {
                let deadline = Instant::now() + Duration::from_millis(timeout as u64);
                loop {
                    let status = inner.child.try_wait().map_err(|error| {
                        Error::operation("sidecar.wait-exit", error.to_string())
                    })?;
                    if let Some(status) = status {
                        return Ok(status.code());
                    }
                    if Instant::now() >= deadline {
                        return Ok(None);
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            },
        }
    }

    fn terminate(&mut self, grace_ms: u32) -> Result<()> {
        let mut inner = self.inner.lock();
        if let Some(_status) = inner
            .child
            .try_wait()
            .map_err(|error| Error::operation("sidecar.terminate", error.to_string()))?
        {
            return Ok(());
        }

        Self::signal_graceful_shutdown(&mut inner);
        if let Some(_status) = inner
            .child
            .try_wait()
            .map_err(|error| Error::operation("sidecar.terminate", error.to_string()))?
        {
            return Ok(());
        }

        if grace_ms > 0 {
            #[cfg(unix)]
            if let Some(group) = self.unix_process_group.as_ref() {
                let _ = group.signal(libc::SIGTERM);
            }
            let deadline = Instant::now() + Duration::from_millis(grace_ms as u64);
            loop {
                if let Some(_status) = inner
                    .child
                    .try_wait()
                    .map_err(|error| Error::operation("sidecar.terminate", error.to_string()))?
                {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }

        #[cfg(unix)]
        if let Some(group) = self.unix_process_group.as_ref() {
            group.signal(libc::SIGKILL)?;
            let _ = inner.child.wait();
            return Ok(());
        }

        inner
            .child
            .kill()
            .map_err(|error| Error::operation("sidecar.terminate", error.to_string()))?;
        let _ = inner.child.wait();
        Ok(())
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        let _ = self.terminate(0);
        for path in self.created_ring_paths.drain(..) {
            let _ = std::fs::remove_file(path);
        }
        #[cfg(unix)]
        for name in self.created_semaphore_names.drain(..) {
            let _ = unlink_named_semaphore(name.as_str());
        }
    }
}

fn build_env_map(env: &[(String, String)]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::<String, String>::new();
    for (key, value) in env {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        map.insert(key.to_ascii_uppercase(), value.clone());
    }
    map
}
