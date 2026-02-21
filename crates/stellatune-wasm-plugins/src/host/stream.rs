use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use crate::error::{Error, Result};

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
    let headers = request.headers.clone();
    let method = request.method.unwrap_or(StreamHttpMethod::Get);
    let body = request.body.clone().unwrap_or_default();
    let connect_timeout = request
        .connect_timeout_ms
        .map(|value| Duration::from_millis(value as u64));
    let read_timeout = request
        .read_timeout_ms
        .map(|value| Duration::from_millis(value as u64));

    let mut client_builder = reqwest::blocking::Client::builder();
    if let Some(connect_timeout) = connect_timeout {
        client_builder = client_builder.connect_timeout(connect_timeout);
    }
    if let Some(read_timeout) = read_timeout {
        client_builder = client_builder.timeout(read_timeout);
    }
    let client = client_builder
        .build()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?;

    let method = match method {
        StreamHttpMethod::Get => reqwest::Method::GET,
        StreamHttpMethod::Post => reqwest::Method::POST,
        StreamHttpMethod::Put => reqwest::Method::PUT,
        StreamHttpMethod::Delete => reqwest::Method::DELETE,
        StreamHttpMethod::Head => reqwest::Method::HEAD,
        StreamHttpMethod::Patch => reqwest::Method::PATCH,
    };

    let mut builder = client.request(method, url);
    for StreamHeader { name, value } in headers {
        builder = builder.header(name, value);
    }
    if !body.is_empty() {
        builder = builder.body(body);
    }

    let response = builder
        .send()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?
        .error_for_status()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?;
    let bytes = response
        .bytes()
        .map_err(|error| Error::operation("host_stream.open", error.to_string()))?
        .to_vec();
    Ok(Box::new(DefaultHostStreamHandle::memory(bytes)))
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
    Memory(Cursor<Vec<u8>>),
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

    fn memory(bytes: Vec<u8>) -> Self {
        Self {
            inner: DefaultHostStreamInner::Memory(Cursor::new(bytes)),
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
            DefaultHostStreamInner::Memory(cursor) => cursor.read(&mut buf)?,
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
            DefaultHostStreamInner::Memory(cursor) => cursor.seek(from).map_err(Error::from),
            DefaultHostStreamInner::Tcp(_) => Err(Error::unsupported(
                "seek is unsupported for tcp streams".to_string(),
            )),
        }
    }

    fn tell(&mut self) -> Result<u64> {
        match &mut self.inner {
            DefaultHostStreamInner::File(file) => file.stream_position().map_err(Error::from),
            DefaultHostStreamInner::Memory(cursor) => cursor.stream_position().map_err(Error::from),
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
            DefaultHostStreamInner::Memory(cursor) => Ok(cursor.get_ref().len() as u64),
            DefaultHostStreamInner::Tcp(_) => Err(Error::unsupported(
                "size is unsupported for tcp streams".to_string(),
            )),
        }
    }
}
