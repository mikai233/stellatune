//! Bounded binary control RPC. Only this worker waits for the driver process;
//! PCM writes and playback clock reads never make synchronous RPC calls.
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use stellatune_asio_proto::{PROTOCOL_VERSION, Request, Response, read_frame, write_frame};

const RPC_TIMEOUT: Duration = Duration::from_secs(3);
type Reply = mpsc::Sender<Result<Response, String>>;

pub(crate) struct Connection {
    commands: mpsc::SyncSender<(Request, Reply)>,
    child: Arc<Mutex<Child>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    pub consumed: Arc<AtomicU64>,
    alive: Arc<AtomicBool>,
}

impl Connection {
    pub fn spawn(executable: &Path, mapping: Option<&Path>) -> Result<Arc<Self>, String> {
        let mut command = Command::new(executable);
        command
            .env("STELLATUNE_SIDECAR_LOG_FILE_PREFIX", "stellatune-asio-host")
            .env(
                "STELLATUNE_SIDECAR_LOG_DIR",
                std::env::var_os("STELLATUNE_ASIO_LOG_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::temp_dir().join("stellatune/asio-logs")),
            )
            .env("STELLATUNE_SIDECAR_LOG_LEVEL", "info");
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(path) = mapping {
            // A dedicated variable accepts paths containing ',' or ';' as well.
            command.env("STELLATUNE_ASIO_PCM_MAPPING", path);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("start ASIO host {}: {e}", executable.display()))?;
        let mut input = child.stdin.take().expect("piped stdin");
        let mut output = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        static GENERATION: AtomicU64 = AtomicU64::new(1);
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
        let stderr_reader = thread::Builder::new()
            .name("asio-stderr".into())
            .spawn(move || {
                use std::io::{BufRead, Read};
                let mut reader = std::io::BufReader::new(stderr);
                let mut bytes = Vec::with_capacity(65536);
                loop {
                    bytes.clear();
                    match (&mut reader).take(65536).read_until(b'\n', &mut bytes) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {},
                    }
                    let line = String::from_utf8_lossy(&bytes);
                    if line.contains(" ERROR ") {
                        tracing::error!(
                            plugin_id = "dev.stellatune.output.asio",
                            generation,
                            "{line}"
                        );
                    } else if line.contains(" WARN ") {
                        tracing::warn!(
                            plugin_id = "dev.stellatune.output.asio",
                            generation,
                            "{line}"
                        );
                    } else {
                        tracing::info!(
                            plugin_id = "dev.stellatune.output.asio",
                            generation,
                            "{line}"
                        );
                    }
                }
            })
            .map_err(|e| {
                let _ = child.kill();
                let _ = child.wait();
                e.to_string()
            })?;
        let child = Arc::new(Mutex::new(child));
        let (responses_tx, responses) = mpsc::channel();
        let reader = thread::Builder::new()
            .name("asio-response".into())
            .spawn(move || {
                loop {
                    let response =
                        read_frame::<_, Response>(&mut output).map_err(|e| e.to_string());
                    let failed = response.is_err();
                    if responses_tx.send(response).is_err() || failed {
                        break;
                    }
                }
            })
            .map_err(|e| {
                terminate(&child);
                e.to_string()
            })?;
        let (commands, receiver) = mpsc::sync_channel::<(Request, Reply)>(8);
        let alive = Arc::new(AtomicBool::new(true));
        let consumed = Arc::new(AtomicU64::new(0));
        let alive_worker = Arc::clone(&alive);
        let consumed_worker = Arc::clone(&consumed);
        let child_worker = Arc::clone(&child);
        let worker = thread::Builder::new()
            .name("asio-control".into())
            .spawn(move || {
                let mut opened = false;
                loop {
                    let (request, reply) = match receiver.recv_timeout(Duration::from_millis(5)) {
                        Ok((request, reply)) => (request, Some(reply)),
                        Err(mpsc::RecvTimeoutError::Timeout) if opened => {
                            (Request::QueryStatus, None)
                        },
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let closing = matches!(request, Request::Close);
                    let opening = matches!(request, Request::Open { .. });
                    let result = write_frame(&mut input, &request)
                        .map_err(|e| e.to_string())
                        .and_then(|()| {
                            responses
                                .recv_timeout(RPC_TIMEOUT)
                                .map_err(|e| format!("ASIO host response: {e}"))?
                        });
                    if let Ok(Response::Status {
                        consumed_frames, ..
                    }) = &result
                    {
                        consumed_worker.store(*consumed_frames, Ordering::Release);
                    }
                    if opening && matches!(result, Ok(Response::Opened { .. })) {
                        opened = true;
                    }
                    let failed = result.is_err()
                        || (reply.is_none() && matches!(result, Ok(Response::Err { .. })));
                    if let Some(reply) = reply {
                        let _ = reply.send(result);
                    }
                    if failed || closing {
                        break;
                    }
                }
                alive_worker.store(false, Ordering::Release);
                terminate(&child_worker);
                let _ = reader.join();
                let _ = stderr_reader.join();
            })
            .map_err(|e| {
                terminate(&child);
                e.to_string()
            })?;
        let connection = Arc::new(Self {
            commands,
            child,
            worker: Mutex::new(Some(worker)),
            consumed,
            alive,
        });
        handshake(|version| connection.request(Request::Hello { version }))?;
        Ok(connection)
    }

    pub fn request(&self, request: Request) -> Result<Response, String> {
        self.check()?;
        let (tx, rx) = mpsc::channel();
        self.commands
            .try_send((request, tx))
            .map_err(|e| format!("ASIO control queue: {e}"))?;
        let response = rx
            .recv_timeout(RPC_TIMEOUT + Duration::from_secs(1))
            .map_err(|e| format!("ASIO control timed out or closed: {e}"))??;
        match response {
            Response::Err { message } => Err(message),
            other => Ok(other),
        }
    }
    pub fn ok(&self, request: Request) -> Result<(), String> {
        match self.request(request)? {
            Response::Ok => Ok(()),
            other => Err(format!("unexpected ASIO response: {other:?}")),
        }
    }
    pub fn check(&self) -> Result<(), String> {
        if self.alive.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err("ASIO host exited or stopped responding".into())
        }
    }
    pub fn close(&self) {
        let mut worker = self
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(join) = worker.take() {
            let _ = self.ok(Request::Close);
            self.alive.store(false, Ordering::Release);
            terminate(&self.child);
            let _ = join.join();
        }
    }
}

fn handshake(hello: impl FnOnce(u32) -> Result<Response, String>) -> Result<(), String> {
    match hello(PROTOCOL_VERSION)? {
        Response::HelloOk { version } if version == PROTOCOL_VERSION => Ok(()),
        response => Err(format!("incompatible ASIO host: {response:?}")),
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.close();
    }
}

fn terminate(child: &Mutex<Child>) {
    let mut child = child
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // kill is harmless after a clean exit and guarantees a stuck driver cannot
    // keep the response reader (or package executable) alive indefinitely.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::{PROTOCOL_VERSION, Response, handshake};

    #[test]
    fn handshake_requires_current_version_without_retrying() {
        handshake(|version| Ok(Response::HelloOk { version })).unwrap();
        let error = handshake(|_| Ok(Response::HelloOk { version: 9 })).unwrap_err();
        assert!(error.contains("incompatible ASIO host"));
        for message in [
            format!("protocol version mismatch: client={PROTOCOL_VERSION}, host=9"),
            "host timed out".into(),
        ] {
            let mut requests = Vec::new();
            let result = handshake(|version| {
                requests.push(version);
                Err(message.clone())
            });
            assert_eq!(result.unwrap_err(), message);
            assert_eq!(requests, [PROTOCOL_VERSION]);
        }
    }
}
