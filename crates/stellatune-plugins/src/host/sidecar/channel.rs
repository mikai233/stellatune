use std::fs::File;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::process::{Child, ChildStdin, ChildStdout};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::error::{Error, Result};

use super::process::ProcessHandle;
use super::shm::SharedMemoryChannelIo;
use super::types::{SidecarChannelHandle, SidecarTransportKind};

pub(super) struct ChildIo {
    pub(super) child: Child,
    pub(super) stdin: Option<ChildStdin>,
    pub(super) stdout: ChildStdout,
}

pub(super) enum ChannelIo {
    Stdio(Arc<Mutex<ChildIo>>),
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
    File(File),
    SharedMemory(SharedMemoryChannelIo),
}

pub(super) struct ChannelHandle {
    transport: SidecarTransportKind,
    io: ChannelIo,
    closed: bool,
}

impl ChannelHandle {
    pub(super) fn stdio(inner: Arc<Mutex<ChildIo>>) -> Self {
        Self {
            transport: SidecarTransportKind::Stdio,
            io: ChannelIo::Stdio(inner),
            closed: false,
        }
    }

    pub(super) fn transport(transport: SidecarTransportKind, io: ChannelIo) -> Self {
        Self {
            transport,
            io,
            closed: false,
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
                let stdin = inner.stdin.as_mut().ok_or_else(|| {
                    Error::operation("sidecar.channel.write", "sidecar stdin is closed")
                })?;
                stdin.write_all(data).map_err(|error| {
                    Error::operation("sidecar.channel.write", error.to_string())
                })?;
                stdin.flush().map_err(|error| {
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

impl ProcessHandle {
    pub(super) fn signal_graceful_shutdown(inner: &mut ChildIo) {
        if let Some(mut stdin) = inner.stdin.take() {
            let _ = stdin.flush();
        }
    }
}
