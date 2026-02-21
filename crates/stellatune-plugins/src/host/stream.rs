use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;

use crate::error::{Error, Result};
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSeekWhence {
    Start,
    Current,
    End,
}

pub trait HostStreamHandle: Send {
    fn read(&mut self, max_bytes: u32) -> Result<Vec<u8>>;
    fn seek(&mut self, offset: i64, whence: StreamSeekWhence) -> Result<u64>;
    fn tell(&mut self) -> Result<u64>;
    fn size(&mut self) -> Result<u64>;
    fn close(&mut self) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOpenKind {
    File,
    Http,
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamHttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Patch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostStreamOpenRequest {
    pub kind: StreamOpenKind,
    pub target: String,
    pub method: Option<StreamHttpMethod>,
    pub headers: Vec<StreamHeader>,
    pub body: Option<Vec<u8>>,
    pub connect_timeout_ms: Option<u32>,
    pub read_timeout_ms: Option<u32>,
}

impl HostStreamOpenRequest {
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            kind: StreamOpenKind::File,
            target: path.into(),
            method: None,
            headers: Vec::new(),
            body: None,
            connect_timeout_ms: None,
            read_timeout_ms: None,
        }
    }

    pub fn http(url: impl Into<String>) -> Self {
        Self {
            kind: StreamOpenKind::Http,
            target: url.into(),
            method: Some(StreamHttpMethod::Get),
            headers: Vec::new(),
            body: None,
            connect_timeout_ms: None,
            read_timeout_ms: None,
        }
    }
}

pub trait HostStreamService: Send + Sync {
    fn open(&self, request: &HostStreamOpenRequest) -> Result<Box<dyn HostStreamHandle>>;
}

#[derive(Default)]
pub struct DefaultHostStreamService;

impl HostStreamService for DefaultHostStreamService {
    fn open(&self, request: &HostStreamOpenRequest) -> Result<Box<dyn HostStreamHandle>> {
        open_default_stream(request)
    }
}

pub fn open_local_file_stream(path: &Path) -> Result<Box<dyn HostStreamHandle>> {
    let file = File::open(path).map_err(|error| Error::io_at(path.to_path_buf(), error))?;
    Ok(Box::new(DefaultHostStreamHandle::file(file)))
}

fn open_default_stream(request: &HostStreamOpenRequest) -> Result<Box<dyn HostStreamHandle>> {
    match request.kind {
        StreamOpenKind::File => open_local_file_stream(Path::new(request.target.trim())),
        StreamOpenKind::Http => open_http_stream(request),
        StreamOpenKind::Tcp => open_tcp_stream(request),
        StreamOpenKind::Udp => Err(Error::unsupported(
            "udp stream transport is not implemented".to_string(),
        )),
    }
}

fn open_http_stream(request: &HostStreamOpenRequest) -> Result<Box<dyn HostStreamHandle>> {
    let url = request.target.trim();
    if url.is_empty() {
        return Err(Error::invalid_input("http target is empty"));
    }
    let config = HttpStreamConfig {
        url: url.to_string(),
        method: request.method.unwrap_or(StreamHttpMethod::Get),
        headers: request.headers.clone(),
        body: request.body.clone().unwrap_or_default(),
        connect_timeout: request
            .connect_timeout_ms
            .map(|value| Duration::from_millis(value as u64)),
        read_timeout: request
            .read_timeout_ms
            .map(|value| Duration::from_millis(value as u64)),
    };

    let (chunk_rx, cancel_tx, worker_join, total_size) = start_http_stream_worker(&config, 0)?;
    Ok(Box::new(HttpHostStreamHandle::new(
        config,
        chunk_rx,
        cancel_tx,
        worker_join,
        total_size,
    )))
}

fn stream_http_chunks(
    response: &mut reqwest::blocking::Response,
    chunk_tx: &SyncSender<HttpChunkMessage>,
    cancel_rx: &mpsc::Receiver<()>,
) {
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        if cancel_rx.try_recv().is_ok() {
            return;
        }

        let read = match response.read(&mut buffer) {
            Ok(0) => {
                let _ = chunk_tx.send(HttpChunkMessage::Eof);
                return;
            },
            Ok(n) => n,
            Err(error) => {
                let _ = chunk_tx.send(HttpChunkMessage::Error(error.to_string()));
                return;
            },
        };
        let mut chunk = vec![0_u8; read];
        chunk.copy_from_slice(&buffer[..read]);
        if chunk_tx.send(HttpChunkMessage::Chunk(chunk)).is_err() {
            return;
        }
    }
}

fn start_http_stream_worker(
    config: &HttpStreamConfig,
    start_offset: u64,
) -> Result<(
    Receiver<HttpChunkMessage>,
    mpsc::Sender<()>,
    thread::JoinHandle<()>,
    Option<u64>,
)> {
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<HttpOpenState>>(1);
    let (chunk_tx, chunk_rx) = mpsc::sync_channel::<HttpChunkMessage>(8);
    let (cancel_tx, cancel_rx) = mpsc::channel::<()>();
    let config = config.clone();

    let worker = thread::Builder::new()
        .name("stellatune-host-http-stream".to_string())
        .spawn(move || {
            let init_result = (|| -> Result<HttpOpenState> {
                let mut response = open_http_response(&config, start_offset)?;
                let state = HttpOpenState {
                    total_size: resolve_total_size(&response, start_offset),
                };
                let _ = ready_tx.send(Ok(state.clone()));
                stream_http_chunks(&mut response, &chunk_tx, &cancel_rx);
                Ok(state)
            })();

            if let Err(error) = init_result {
                let _ = ready_tx.send(Err(error));
            }
        })
        .map_err(|error| {
            Error::operation(
                "host_stream.open",
                format!("spawn http stream worker failed: {error}"),
            )
        })?;

    let state = ready_rx
        .recv()
        .map_err(|_| Error::operation("host_stream.open", "http worker did not report status"))??;
    Ok((chunk_rx, cancel_tx, worker, state.total_size))
}

fn open_http_response(
    config: &HttpStreamConfig,
    start_offset: u64,
) -> Result<reqwest::blocking::Response> {
    let mut client_builder = reqwest::blocking::Client::builder();
    if let Some(connect_timeout) = config.connect_timeout {
        client_builder = client_builder.connect_timeout(connect_timeout);
    }
    if let Some(read_timeout) = config.read_timeout {
        client_builder = client_builder.timeout(read_timeout);
    }
    let client = client_builder
        .build()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?;

    let method = match config.method {
        StreamHttpMethod::Get => reqwest::Method::GET,
        StreamHttpMethod::Post => reqwest::Method::POST,
        StreamHttpMethod::Put => reqwest::Method::PUT,
        StreamHttpMethod::Delete => reqwest::Method::DELETE,
        StreamHttpMethod::Head => reqwest::Method::HEAD,
        StreamHttpMethod::Patch => reqwest::Method::PATCH,
    };

    let mut builder = client.request(method, config.url.as_str());
    for StreamHeader { name, value } in &config.headers {
        if start_offset > 0 && name.eq_ignore_ascii_case("range") {
            continue;
        }
        builder = builder.header(name.as_str(), value.as_str());
    }
    if !config.body.is_empty() {
        builder = builder.body(config.body.clone());
    }
    if start_offset > 0 {
        builder = builder.header("Range", format!("bytes={start_offset}-"));
    }

    let response = builder
        .send()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?
        .error_for_status()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?;
    if start_offset > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        return Err(Error::unsupported(format!(
            "http server does not support range seek for {}",
            config.url
        )));
    }
    Ok(response)
}

fn resolve_total_size(response: &reqwest::blocking::Response, start_offset: u64) -> Option<u64> {
    if let Some(content_range) = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        && let Some(total) = parse_content_range_total(content_range)
    {
        return Some(total);
    }

    if start_offset == 0 {
        return response.content_length();
    }
    response
        .content_length()
        .map(|remaining| start_offset.saturating_add(remaining))
}

fn parse_content_range_total(content_range: &str) -> Option<u64> {
    let (_, total) = content_range.split_once('/')?;
    let total = total.trim();
    if total == "*" {
        return None;
    }
    total.parse::<u64>().ok()
}

fn open_tcp_stream(request: &HostStreamOpenRequest) -> Result<Box<dyn HostStreamHandle>> {
    let target = request.target.trim();
    if target.is_empty() {
        return Err(Error::invalid_input("tcp target is empty"));
    }
    let timeout = request
        .connect_timeout_ms
        .map(|value| Duration::from_millis(value as u64))
        .unwrap_or_else(|| Duration::from_secs(10));
    let addr = target
        .to_socket_addrs()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?
        .next()
        .ok_or_else(|| Error::invalid_input("tcp target resolved to no addresses"))?;

    let stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?;
    if let Some(read_timeout_ms) = request.read_timeout_ms {
        stream
            .set_read_timeout(Some(Duration::from_millis(read_timeout_ms as u64)))
            .map_err(|error| Error::operation("host_stream.open", error.to_string()))?;
    }
    Ok(Box::new(DefaultHostStreamHandle::tcp(stream)))
}

enum DefaultHostStreamInner {
    File(File),
    Tcp(TcpStream),
}

struct DefaultHostStreamHandle {
    inner: DefaultHostStreamInner,
}

impl DefaultHostStreamHandle {
    fn file(file: File) -> Self {
        Self {
            inner: DefaultHostStreamInner::File(file),
        }
    }

    fn tcp(stream: TcpStream) -> Self {
        Self {
            inner: DefaultHostStreamInner::Tcp(stream),
        }
    }
}

impl HostStreamHandle for DefaultHostStreamHandle {
    fn read(&mut self, max_bytes: u32) -> Result<Vec<u8>> {
        let max_bytes = max_bytes.max(1) as usize;
        let mut buf = vec![0u8; max_bytes];
        let read = match &mut self.inner {
            DefaultHostStreamInner::File(file) => file.read(&mut buf)?,
            DefaultHostStreamInner::Tcp(stream) => stream.read(&mut buf)?,
        };
        buf.truncate(read);
        Ok(buf)
    }

    fn seek(&mut self, offset: i64, whence: StreamSeekWhence) -> Result<u64> {
        let from = match whence {
            StreamSeekWhence::Start => {
                if offset < 0 {
                    return Err(Error::invalid_input("negative offset with seek start"));
                }
                SeekFrom::Start(offset as u64)
            },
            StreamSeekWhence::Current => SeekFrom::Current(offset),
            StreamSeekWhence::End => SeekFrom::End(offset),
        };
        match &mut self.inner {
            DefaultHostStreamInner::File(file) => file.seek(from).map_err(Error::from),
            DefaultHostStreamInner::Tcp(_) => Err(Error::unsupported(
                "seek is unsupported for tcp streams".to_string(),
            )),
        }
    }

    fn tell(&mut self) -> Result<u64> {
        match &mut self.inner {
            DefaultHostStreamInner::File(file) => file.stream_position().map_err(Error::from),
            DefaultHostStreamInner::Tcp(_) => Err(Error::unsupported(
                "tell is unsupported for tcp streams".to_string(),
            )),
        }
    }

    fn size(&mut self) -> Result<u64> {
        match &mut self.inner {
            DefaultHostStreamInner::File(file) => {
                file.metadata().map(|meta| meta.len()).map_err(Error::from)
            },
            DefaultHostStreamInner::Tcp(_) => Err(Error::unsupported(
                "size is unsupported for tcp streams".to_string(),
            )),
        }
    }
}

enum HttpChunkMessage {
    Chunk(Vec<u8>),
    Eof,
    Error(String),
}

#[derive(Debug, Clone)]
struct HttpOpenState {
    total_size: Option<u64>,
}

#[derive(Debug, Clone)]
struct HttpStreamConfig {
    url: String,
    method: StreamHttpMethod,
    headers: Vec<StreamHeader>,
    body: Vec<u8>,
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
}

struct HttpHostStreamHandle {
    config: HttpStreamConfig,
    chunk_rx: Receiver<HttpChunkMessage>,
    cancel_tx: Option<mpsc::Sender<()>>,
    worker_join: Option<thread::JoinHandle<()>>,
    total_size: Option<u64>,
    pending: Vec<u8>,
    pending_offset: usize,
    position: u64,
    eof: bool,
}

impl HttpHostStreamHandle {
    fn new(
        config: HttpStreamConfig,
        chunk_rx: Receiver<HttpChunkMessage>,
        cancel_tx: mpsc::Sender<()>,
        worker_join: thread::JoinHandle<()>,
        total_size: Option<u64>,
    ) -> Self {
        Self {
            config,
            chunk_rx,
            cancel_tx: Some(cancel_tx),
            worker_join: Some(worker_join),
            total_size,
            pending: Vec::new(),
            pending_offset: 0,
            position: 0,
            eof: false,
        }
    }

    fn drain_pending_into(&mut self, out: &mut Vec<u8>, max_bytes: usize) {
        if self.pending_offset >= self.pending.len() {
            return;
        }
        let remain = self.pending.len() - self.pending_offset;
        let take = remain.min(max_bytes.saturating_sub(out.len()));
        if take == 0 {
            return;
        }
        let start = self.pending_offset;
        let end = start + take;
        out.extend_from_slice(&self.pending[start..end]);
        self.pending_offset = end;
        self.position = self.position.saturating_add(take as u64);
        if self.pending_offset >= self.pending.len() {
            self.pending.clear();
            self.pending_offset = 0;
        }
    }

    fn close_worker(&mut self) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }
        let _ = self.worker_join.take();
    }

    fn reopen_at(&mut self, position: u64) -> Result<u64> {
        if !matches!(self.config.method, StreamHttpMethod::Get) {
            return Err(Error::unsupported(
                "http seek is only supported for GET streams".to_string(),
            ));
        }

        self.close_worker();
        let (chunk_rx, cancel_tx, worker_join, total_size) =
            start_http_stream_worker(&self.config, position)?;
        self.chunk_rx = chunk_rx;
        self.cancel_tx = Some(cancel_tx);
        self.worker_join = Some(worker_join);
        self.total_size = total_size;
        self.pending.clear();
        self.pending_offset = 0;
        self.position = position;
        self.eof = false;
        Ok(position)
    }

    fn seek_target(&self, offset: i64, whence: StreamSeekWhence) -> Result<u64> {
        match whence {
            StreamSeekWhence::Start => {
                if offset < 0 {
                    return Err(Error::invalid_input("negative offset with seek start"));
                }
                Ok(offset as u64)
            },
            StreamSeekWhence::Current => {
                if offset >= 0 {
                    self.position
                        .checked_add(offset as u64)
                        .ok_or_else(|| Error::invalid_input("seek overflow with current base"))
                } else {
                    self.position
                        .checked_sub(offset.unsigned_abs())
                        .ok_or_else(|| Error::invalid_input("seek underflow with current base"))
                }
            },
            StreamSeekWhence::End => {
                let end = self.total_size.ok_or_else(|| {
                    Error::unsupported(
                        "seek from end is unavailable for unknown http stream size".to_string(),
                    )
                })?;
                if offset >= 0 {
                    end.checked_add(offset as u64)
                        .ok_or_else(|| Error::invalid_input("seek overflow with end base"))
                } else {
                    end.checked_sub(offset.unsigned_abs())
                        .ok_or_else(|| Error::invalid_input("seek underflow with end base"))
                }
            },
        }
    }

    fn close_inner(&mut self) {
        self.close_worker();
    }

    fn log_seek_error(
        &self,
        target: Option<u64>,
        offset: i64,
        whence: StreamSeekWhence,
        error: &Error,
    ) {
        match error {
            Error::Unsupported { message } => {
                warn!(
                    target: "stellatune_plugins::host::stream",
                    url = %self.config.url,
                    offset,
                    whence = ?whence,
                    seek_target = ?target,
                    reason = %message,
                    "http seek unsupported"
                );
            },
            _ => {
                warn!(
                    target: "stellatune_plugins::host::stream",
                    url = %self.config.url,
                    offset,
                    whence = ?whence,
                    seek_target = ?target,
                    error = %error,
                    "http seek failed"
                );
            },
        }
    }
}

impl HostStreamHandle for HttpHostStreamHandle {
    fn read(&mut self, max_bytes: u32) -> Result<Vec<u8>> {
        let limit = max_bytes.max(1) as usize;
        let mut out = Vec::<u8>::with_capacity(limit);

        loop {
            self.drain_pending_into(&mut out, limit);
            if out.len() >= limit || self.eof {
                break;
            }

            match self.chunk_rx.recv() {
                Ok(HttpChunkMessage::Chunk(chunk)) => {
                    self.pending = chunk;
                    self.pending_offset = 0;
                },
                Ok(HttpChunkMessage::Eof) => {
                    self.eof = true;
                },
                Ok(HttpChunkMessage::Error(message)) => {
                    return Err(Error::operation("host_stream.read", message));
                },
                Err(_) => {
                    self.eof = true;
                },
            }
        }

        Ok(out)
    }

    fn seek(&mut self, offset: i64, whence: StreamSeekWhence) -> Result<u64> {
        let target = match self.seek_target(offset, whence) {
            Ok(value) => value,
            Err(error) => {
                self.log_seek_error(None, offset, whence, &error);
                return Err(error);
            },
        };
        if target == self.position {
            return Ok(self.position);
        }
        if let Err(error) = self.reopen_at(target) {
            self.log_seek_error(Some(target), offset, whence, &error);
            return Err(error);
        }
        Ok(target)
    }

    fn tell(&mut self) -> Result<u64> {
        Ok(self.position)
    }

    fn size(&mut self) -> Result<u64> {
        self.total_size.ok_or_else(|| {
            Error::unsupported("size is unavailable for chunked http streams".to_string())
        })
    }

    fn close(&mut self) {
        self.close_inner();
    }
}

impl Drop for HttpHostStreamHandle {
    fn drop(&mut self) {
        self.close_inner();
    }
}
