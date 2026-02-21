use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::mem;
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use memmap2::{MmapMut, MmapOptions};
use parking_lot::Mutex;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

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
    pub executable: String,
    pub args: Vec<String>,
    pub preferred_control: Vec<SidecarTransportOption>,
    pub preferred_data: Vec<SidecarTransportOption>,
    pub env: Vec<(String, String)>,
}

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

pub(crate) fn default_sidecar_host() -> Arc<dyn SidecarHost> {
    Arc::new(ProcessSidecarHost)
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

    // On Windows plugin configs often use a bare executable name without ".exe".
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
        prepare_shared_memory_env(
            &spec.preferred_control,
            "STELLATUNE_SIDECAR_CONTROL_SHARED_MEMORY_RING",
            "STELLATUNE_SIDECAR_CONTROL_SHM",
            &mut env,
            &mut env_map,
            &mut created_ring_paths,
        )?;
        prepare_shared_memory_env(
            &spec.preferred_data,
            "STELLATUNE_SIDECAR_DATA_SHARED_MEMORY_RING",
            "STELLATUNE_SIDECAR_DATA_SHM",
            &mut env,
            &mut env_map,
            &mut created_ring_paths,
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

        let mut child = command
            .spawn()
            .map_err(|error| Error::operation("sidecar.launch", error.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::operation("sidecar.launch", "missing stdin pipe"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::operation("sidecar.launch", "missing stdout pipe"))?;

        Ok(Box::new(ProcessHandle {
            inner: Arc::new(Mutex::new(ChildIo {
                child,
                stdin,
                stdout,
            })),
            control_preferred: spec.preferred_control.clone(),
            data_preferred: spec.preferred_data.clone(),
            env_map,
            created_ring_paths,
        }))
    }
}

struct ChildIo {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

struct ProcessHandle {
    inner: Arc<Mutex<ChildIo>>,
    control_preferred: Vec<SidecarTransportOption>,
    data_preferred: Vec<SidecarTransportOption>,
    env_map: BTreeMap<String, String>,
    created_ring_paths: Vec<PathBuf>,
}

enum ChannelIo {
    Stdio(Arc<Mutex<ChildIo>>),
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
    File(File),
    SharedMemory(SharedMemoryChannelIo),
}

struct ChannelHandle {
    transport: SidecarTransportKind,
    io: ChannelIo,
    closed: bool,
}

impl ChannelHandle {
    fn stdio(inner: Arc<Mutex<ChildIo>>) -> Self {
        Self {
            transport: SidecarTransportKind::Stdio,
            io: ChannelIo::Stdio(inner),
            closed: false,
        }
    }

    fn transport(transport: SidecarTransportKind, io: ChannelIo) -> Self {
        Self {
            transport,
            io,
            closed: false,
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
                let stream = TcpStream::connect(endpoint.as_str()).map_err(|error| {
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
                let file = OpenOptions::new()
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
                    let stream = UnixStream::connect(endpoint.as_str()).map_err(|error| {
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

        if grace_ms > 0 {
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
    }
}

impl SidecarChannelHandle for ChannelHandle {
    fn transport(&self) -> SidecarTransportKind {
        self.transport
    }

    fn write(&mut self, data: &[u8]) -> Result<u32> {
        if self.closed {
            return Err(Error::operation(
                "sidecar.channel.write",
                "channel is closed",
            ));
        }

        match &mut self.io {
            ChannelIo::Stdio(inner) => {
                let mut inner = inner.lock();
                inner.stdin.write_all(data).map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
                inner.stdin.flush().map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
            },
            ChannelIo::Tcp(stream) => {
                stream.write_all(data).map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
                stream.flush().map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
            },
            #[cfg(unix)]
            ChannelIo::Unix(stream) => {
                stream.write_all(data).map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
                stream.flush().map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
            },
            ChannelIo::File(file) => {
                file.write_all(data).map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
                file.flush().map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
            },
            ChannelIo::SharedMemory(shared) => return shared.write(data),
        }
        Ok(data.len() as u32)
    }

    fn read(&mut self, max_bytes: u32, timeout_ms: Option<u32>) -> Result<Vec<u8>> {
        if self.closed || max_bytes == 0 {
            return Ok(Vec::new());
        }

        let mut buffer = vec![0_u8; max_bytes as usize];
        let size = match &mut self.io {
            ChannelIo::Stdio(inner) => {
                let mut inner = inner.lock();
                inner
                    .stdout
                    .read(&mut buffer)
                    .map_err(|error| Error::operation("sidecar.channel.read", error.to_string()))?
            },
            ChannelIo::Tcp(stream) => {
                let _ =
                    stream.set_read_timeout(timeout_ms.map(|ms| Duration::from_millis(ms as u64)));
                stream
                    .read(&mut buffer)
                    .map_err(|error| Error::operation("sidecar.channel.read", error.to_string()))?
            },
            #[cfg(unix)]
            ChannelIo::Unix(stream) => {
                let _ =
                    stream.set_read_timeout(timeout_ms.map(|ms| Duration::from_millis(ms as u64)));
                stream
                    .read(&mut buffer)
                    .map_err(|error| Error::operation("sidecar.channel.read", error.to_string()))?
            },
            ChannelIo::File(file) => file
                .read(&mut buffer)
                .map_err(|error| Error::operation("sidecar.channel.read", error.to_string()))?,
            ChannelIo::SharedMemory(shared) => return shared.read(max_bytes, timeout_ms),
        };
        buffer.truncate(size);
        Ok(buffer)
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        match &mut self.io {
            ChannelIo::Tcp(stream) => {
                let _ = stream.shutdown(Shutdown::Both);
            },
            #[cfg(unix)]
            ChannelIo::Unix(stream) => {
                let _ = stream.shutdown(Shutdown::Both);
            },
            ChannelIo::Stdio(_) | ChannelIo::File(_) | ChannelIo::SharedMemory(_) => {},
        }
    }
}

const SHM_MAGIC: u32 = 0x53544D52; // "STMR"
const SHM_VERSION: u32 = 1;
const SHM_MIN_CAPACITY: usize = 4 * 1024;
const SHM_MAX_CAPACITY: usize = 64 * 1024 * 1024;
const SHM_DEFAULT_CAPACITY: usize = 1024 * 1024;
const SHM_POLL_INTERVAL: Duration = Duration::from_millis(1);

#[repr(C)]
struct SharedByteRingHeader {
    magic: u32,
    version: u32,
    capacity_bytes: u32,
    _reserved: u32,
    write_pos: AtomicU64,
    read_pos: AtomicU64,
}

struct SharedByteRingMapped {
    map: MmapMut,
    capacity_bytes: usize,
}

impl SharedByteRingMapped {
    fn header_size() -> usize {
        mem::size_of::<SharedByteRingHeader>()
    }

    fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| Error::operation("sidecar.shm.open", error.to_string()))?;
        let map = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|error| Error::operation("sidecar.shm.map", error.to_string()))?
        };
        if map.len() < Self::header_size() {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!("ring file too small: {}", path.display()),
            ));
        }

        let header = unsafe { &*(map.as_ptr() as *const SharedByteRingHeader) };
        if header.magic != SHM_MAGIC || header.version != SHM_VERSION {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!("invalid ring header: {}", path.display()),
            ));
        }
        let capacity_bytes = header.capacity_bytes as usize;
        if !(SHM_MIN_CAPACITY..=SHM_MAX_CAPACITY).contains(&capacity_bytes) {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!(
                    "invalid ring capacity {} for {}",
                    capacity_bytes,
                    path.display()
                ),
            ));
        }
        let expected = Self::header_size()
            .checked_add(capacity_bytes)
            .ok_or_else(|| Error::operation("sidecar.shm.open", "capacity overflow"))?;
        if expected != map.len() {
            return Err(Error::operation(
                "sidecar.shm.open",
                format!(
                    "ring size mismatch for {}: expect {}, got {}",
                    path.display(),
                    expected,
                    map.len()
                ),
            ));
        }
        Ok(Self {
            map,
            capacity_bytes,
        })
    }

    fn create(path: &Path, capacity_bytes: usize) -> Result<Self> {
        if !(SHM_MIN_CAPACITY..=SHM_MAX_CAPACITY).contains(&capacity_bytes) {
            return Err(Error::operation(
                "sidecar.shm.create",
                format!("invalid capacity {}", capacity_bytes),
            ));
        }
        let total = Self::header_size()
            .checked_add(capacity_bytes)
            .ok_or_else(|| Error::operation("sidecar.shm.create", "capacity overflow"))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| Error::operation("sidecar.shm.create", error.to_string()))?;
        file.set_len(total as u64)
            .map_err(|error| Error::operation("sidecar.shm.create", error.to_string()))?;
        let mut map = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|error| Error::operation("sidecar.shm.create", error.to_string()))?
        };
        unsafe {
            let header = map.as_mut_ptr() as *mut SharedByteRingHeader;
            ptr::write(
                header,
                SharedByteRingHeader {
                    magic: SHM_MAGIC,
                    version: SHM_VERSION,
                    capacity_bytes: capacity_bytes as u32,
                    _reserved: 0,
                    write_pos: AtomicU64::new(0),
                    read_pos: AtomicU64::new(0),
                },
            );
            ptr::write_bytes(map.as_mut_ptr().add(Self::header_size()), 0, capacity_bytes);
        }
        Ok(Self {
            map,
            capacity_bytes,
        })
    }

    fn header(&self) -> &SharedByteRingHeader {
        unsafe { &*(self.map.as_ptr() as *const SharedByteRingHeader) }
    }

    fn write_bytes(&mut self, input: &[u8]) -> usize {
        if input.is_empty() {
            return 0;
        }
        let header = self.header();
        let read_pos = header.read_pos.load(Ordering::Acquire);
        let write_pos = header.write_pos.load(Ordering::Relaxed);
        let used = write_pos
            .saturating_sub(read_pos)
            .min(self.capacity_bytes as u64) as usize;
        let available = self.capacity_bytes.saturating_sub(used);
        let count = available.min(input.len());
        if count == 0 {
            return 0;
        }

        let start = (write_pos as usize) % self.capacity_bytes;
        let first = count.min(self.capacity_bytes - start);
        unsafe {
            let base = self.map.as_ptr() as *mut u8;
            let data = base.add(Self::header_size());
            ptr::copy_nonoverlapping(input.as_ptr(), data.add(start), first);
            if first < count {
                ptr::copy_nonoverlapping(input.as_ptr().add(first), data, count - first);
            }
        }
        header
            .write_pos
            .store(write_pos + count as u64, Ordering::Release);
        count
    }

    fn read_bytes(&mut self, out: &mut [u8]) -> usize {
        if out.is_empty() {
            return 0;
        }
        let header = self.header();
        let write_pos = header.write_pos.load(Ordering::Acquire);
        let read_pos = header.read_pos.load(Ordering::Relaxed);
        let available = write_pos
            .saturating_sub(read_pos)
            .min(self.capacity_bytes as u64) as usize;
        let count = available.min(out.len());
        if count == 0 {
            return 0;
        }

        let start = (read_pos as usize) % self.capacity_bytes;
        let first = count.min(self.capacity_bytes - start);
        unsafe {
            let base = self.map.as_ptr();
            let data = base.add(Self::header_size());
            ptr::copy_nonoverlapping(data.add(start), out.as_mut_ptr(), first);
            if first < count {
                ptr::copy_nonoverlapping(data, out.as_mut_ptr().add(first), count - first);
            }
        }
        header
            .read_pos
            .store(read_pos + count as u64, Ordering::Release);
        count
    }
}

struct SharedMemoryChannelIo {
    tx: SharedByteRingMapped,
    rx: SharedByteRingMapped,
}

impl SharedMemoryChannelIo {
    fn open(endpoint: &str) -> Result<Self> {
        let config = parse_shared_memory_endpoint(endpoint)?;
        let tx = SharedByteRingMapped::open(Path::new(config.tx_path.as_str()))?;
        let rx = SharedByteRingMapped::open(Path::new(config.rx_path.as_str()))?;
        Ok(Self { tx, rx })
    }

    fn write(&mut self, data: &[u8]) -> Result<u32> {
        Ok(self.tx.write_bytes(data) as u32)
    }

    fn read(&mut self, max_bytes: u32, timeout_ms: Option<u32>) -> Result<Vec<u8>> {
        let max_bytes = max_bytes as usize;
        if max_bytes == 0 {
            return Ok(Vec::new());
        }

        let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms as u64));
        let mut out = vec![0_u8; max_bytes];
        let mut spins = 0;
        let mut yields = 0;

        loop {
            let read = self.rx.read_bytes(&mut out);
            if read > 0 {
                out.truncate(read);
                return Ok(out);
            }

            if let Some(deadline) = deadline {
                if Instant::now() >= deadline {
                    return Ok(Vec::new());
                }
            } else {
                return Ok(Vec::new());
            }

            if spins < 10 {
                std::hint::spin_loop();
                spins += 1;
            } else if yields < 50 {
                thread::yield_now();
                yields += 1;
            } else {
                thread::sleep(SHM_POLL_INTERVAL);
            }
        }
    }
}

struct SharedMemoryEndpoint {
    tx_path: String,
    rx_path: String,
}

fn parse_shared_memory_endpoint(endpoint: &str) -> Result<SharedMemoryEndpoint> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err(Error::invalid_input("shared-memory endpoint is empty"));
    }

    if !endpoint.contains('=') {
        return Ok(SharedMemoryEndpoint {
            tx_path: endpoint.to_string(),
            rx_path: endpoint.to_string(),
        });
    }

    let mut tx_path = None::<String>;
    let mut rx_path = None::<String>;
    for part in endpoint.split([';', ',']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((raw_key, raw_value)) = part.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value.trim();
        if value.is_empty() {
            continue;
        }
        if key == "tx" || key == "write" || key == "host_to_sidecar" {
            tx_path = Some(value.to_string());
        } else if key == "rx" || key == "read" || key == "sidecar_to_host" {
            rx_path = Some(value.to_string());
        } else if key == "path" || key == "ring" {
            let value = value.to_string();
            if tx_path.is_none() {
                tx_path = Some(value.clone());
            }
            if rx_path.is_none() {
                rx_path = Some(value);
            }
        }
    }

    let tx_path = tx_path
        .or_else(|| rx_path.clone())
        .ok_or_else(|| Error::invalid_input("shared-memory endpoint missing `tx` or `path`"))?;
    let rx_path = rx_path
        .or_else(|| Some(tx_path.clone()))
        .ok_or_else(|| Error::invalid_input("shared-memory endpoint missing `rx` or `path`"))?;
    Ok(SharedMemoryEndpoint { tx_path, rx_path })
}

fn prepare_shared_memory_env(
    preferred: &[SidecarTransportOption],
    full_key: &'static str,
    short_key: &'static str,
    env: &mut Vec<(String, String)>,
    env_map: &mut BTreeMap<String, String>,
    created_ring_paths: &mut Vec<PathBuf>,
) -> Result<()> {
    if !preferred
        .iter()
        .any(|option| option.kind == SidecarTransportKind::SharedMemoryRing)
    {
        return Ok(());
    }

    if let Some(value) = first_non_empty_env(env_map, &[full_key, short_key]) {
        ensure_env_entry(env, full_key, &value);
        ensure_env_entry(env, short_key, &value);
        env_map.insert(full_key.to_string(), value.clone());
        env_map.insert(short_key.to_string(), value);
        return Ok(());
    }

    let capacity = preferred
        .iter()
        .filter(|option| option.kind == SidecarTransportKind::SharedMemoryRing)
        .filter_map(|option| option.max_frame_bytes)
        .map(|bytes| bytes as usize)
        .max()
        .unwrap_or(SHM_DEFAULT_CAPACITY)
        .clamp(SHM_MIN_CAPACITY, SHM_MAX_CAPACITY);
    let endpoint = create_shared_memory_endpoint(capacity, created_ring_paths)?;

    ensure_env_entry(env, full_key, endpoint.as_str());
    ensure_env_entry(env, short_key, endpoint.as_str());
    env_map.insert(full_key.to_string(), endpoint.clone());
    env_map.insert(short_key.to_string(), endpoint);
    Ok(())
}

fn create_shared_memory_endpoint(
    capacity_bytes: usize,
    created_ring_paths: &mut Vec<PathBuf>,
) -> Result<String> {
    let base_dir = std::env::temp_dir().join("stellatune-sidecar-shm");
    std::fs::create_dir_all(base_dir.as_path())
        .map_err(|error| Error::operation("sidecar.shm.create-dir", error.to_string()))?;

    let tx_path = unique_ring_path(base_dir.as_path(), "tx");
    let rx_path = unique_ring_path(base_dir.as_path(), "rx");
    let _ = SharedByteRingMapped::create(tx_path.as_path(), capacity_bytes)?;
    let _ = SharedByteRingMapped::create(rx_path.as_path(), capacity_bytes)?;
    created_ring_paths.push(tx_path.clone());
    created_ring_paths.push(rx_path.clone());

    Ok(format!(
        "tx={};rx={}",
        tx_path.to_string_lossy(),
        rx_path.to_string_lossy()
    ))
}

fn unique_ring_path(base_dir: &Path, direction: &str) -> PathBuf {
    let pid = std::process::id();
    let epoch_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    for attempt in 0..1024_u32 {
        let path = base_dir.join(format!("ring-{pid}-{epoch_ns}-{direction}-{attempt}.shm"));
        if !path.exists() {
            return path;
        }
    }
    base_dir.join(format!("ring-{pid}-{epoch_ns}-{direction}.shm"))
}

fn first_non_empty_env(env_map: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| env_map.get(*key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn ensure_env_entry(env: &mut Vec<(String, String)>, key: &str, value: &str) {
    if let Some(entry) = env
        .iter_mut()
        .find(|(existing_key, _)| existing_key.eq_ignore_ascii_case(key))
    {
        entry.0 = key.to_string();
        entry.1 = value.to_string();
        return;
    }
    env.push((key.to_string(), value.to_string()));
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

fn normalize_role_key(role: &str) -> String {
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

fn transport_env_suffixes(kind: SidecarTransportKind) -> &'static [&'static str] {
    match kind {
        SidecarTransportKind::Stdio => &["STDIO"],
        SidecarTransportKind::NamedPipe => &["NAMED_PIPE", "PIPE"],
        SidecarTransportKind::UnixSocket => &["UNIX_SOCKET", "UNIX"],
        SidecarTransportKind::LoopbackTcp => &["LOOPBACK_TCP", "TCP"],
        SidecarTransportKind::SharedMemoryRing => &["SHARED_MEMORY_RING", "SHM"],
    }
}

fn ordered_kinds(options: &[SidecarTransportOption]) -> Vec<SidecarTransportKind> {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::host::sidecar::{
        SHM_MIN_CAPACITY, SharedByteRingMapped, SidecarTransportKind, SidecarTransportOption,
        parse_shared_memory_endpoint, prepare_shared_memory_env, resolve_sidecar_executable,
    };

    #[test]
    fn parse_shared_memory_endpoint_supports_pair_format() {
        let endpoint = parse_shared_memory_endpoint("tx=C:/tmp/a.shm;rx=C:/tmp/b.shm")
            .expect("endpoint parse should succeed");
        assert_eq!(endpoint.tx_path, "C:/tmp/a.shm");
        assert_eq!(endpoint.rx_path, "C:/tmp/b.shm");
    }

    #[test]
    fn parse_shared_memory_endpoint_supports_single_path() {
        let endpoint =
            parse_shared_memory_endpoint("/tmp/ring.shm").expect("single path should be accepted");
        assert_eq!(endpoint.tx_path, "/tmp/ring.shm");
        assert_eq!(endpoint.rx_path, "/tmp/ring.shm");
    }

    #[test]
    fn shared_byte_ring_write_read_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ring.shm");
        let mut writer =
            SharedByteRingMapped::create(path.as_path(), SHM_MIN_CAPACITY).expect("create ring");
        let mut reader = SharedByteRingMapped::open(path.as_path()).expect("open ring");

        let payload = b"stellatune-sidecar-shm";
        let wrote = writer.write_bytes(payload);
        assert_eq!(wrote, payload.len());

        let mut out = vec![0_u8; payload.len()];
        let read = reader.read_bytes(&mut out);
        assert_eq!(read, payload.len());
        assert_eq!(out, payload);
    }

    #[test]
    fn prepare_shared_memory_env_creates_endpoint_files() {
        let preferred = vec![SidecarTransportOption {
            kind: SidecarTransportKind::SharedMemoryRing,
            priority: 10,
            max_frame_bytes: Some(8192),
        }];
        let mut env = Vec::<(String, String)>::new();
        let mut env_map = BTreeMap::<String, String>::new();
        let mut created_paths = Vec::new();

        prepare_shared_memory_env(
            &preferred,
            "STELLATUNE_SIDECAR_DATA_SHARED_MEMORY_RING",
            "STELLATUNE_SIDECAR_DATA_SHM",
            &mut env,
            &mut env_map,
            &mut created_paths,
        )
        .expect("prepare env");

        let endpoint = env_map
            .get("STELLATUNE_SIDECAR_DATA_SHARED_MEMORY_RING")
            .expect("full key must exist");
        assert!(endpoint.contains("tx="));
        assert!(endpoint.contains("rx="));
        assert_eq!(env_map.get("STELLATUNE_SIDECAR_DATA_SHM"), Some(endpoint));

        assert_eq!(created_paths.len(), 2);
        for path in created_paths {
            assert!(path.exists());
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn resolves_bare_executable_in_plugin_bin_dir() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let root = temp.path();
        let bin_dir = root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("create bin dir");

        let bare = "stellatune-asio-host";
        let expected = if cfg!(windows) {
            bin_dir.join("stellatune-asio-host.exe")
        } else {
            bin_dir.join("stellatune-asio-host")
        };
        std::fs::write(&expected, b"stub").expect("create sidecar stub");

        let resolved = resolve_sidecar_executable(root, bare).expect("resolve executable");
        assert_eq!(std::path::Path::new(&resolved), expected.as_path());
    }

    #[test]
    fn fails_when_no_candidate_exists() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let err = resolve_sidecar_executable(temp.path(), "stellatune-asio-host")
            .expect_err("missing sidecar should fail");
        assert!(err.to_string().contains("sidecar executable"));
    }

    #[test]
    fn rejects_parent_dir_relative_path() {
        let temp = tempfile::tempdir().expect("create tempdir");
        let err = resolve_sidecar_executable(temp.path(), "../stellatune-asio-host")
            .expect_err("unsafe relative path should fail");
        assert!(err.to_string().contains("unsafe"));
    }
}
