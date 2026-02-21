use crate::capabilities::DecoderInputStream;
use crate::common::SeekWhence;
use crate::error::{SdkError, SdkResult};

pub trait HostStreamHandle: Send {
    fn read(&mut self, max_bytes: u32) -> SdkResult<Vec<u8>>;
    fn seek(&mut self, offset: i64, whence: SeekWhence) -> SdkResult<u64>;
    fn tell(&mut self) -> SdkResult<u64>;
    fn size(&mut self) -> SdkResult<u64>;
    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOpenKind {
    File,
    Http,
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
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
    pub method: Option<HttpMethod>,
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
            method: Some(HttpMethod::Get),
            headers: Vec::new(),
            body: None,
            connect_timeout_ms: None,
            read_timeout_ms: None,
        }
    }

    pub fn tcp(target: impl Into<String>) -> Self {
        Self {
            kind: StreamOpenKind::Tcp,
            target: target.into(),
            method: None,
            headers: Vec::new(),
            body: None,
            connect_timeout_ms: None,
            read_timeout_ms: None,
        }
    }

    pub fn udp(target: impl Into<String>) -> Self {
        Self {
            kind: StreamOpenKind::Udp,
            target: target.into(),
            method: None,
            headers: Vec::new(),
            body: None,
            connect_timeout_ms: None,
            read_timeout_ms: None,
        }
    }
}

pub trait HostStreamClient {
    type Handle: HostStreamHandle;
    fn open(&mut self, request: &HostStreamOpenRequest) -> SdkResult<Self::Handle>;
}

pub struct HostStreamReader<'a, T: HostStreamHandle + ?Sized> {
    inner: &'a mut T,
}

impl<'a, T: HostStreamHandle + ?Sized> HostStreamReader<'a, T> {
    pub fn new(inner: &'a mut T) -> Self {
        Self { inner }
    }

    pub fn read_chunk(&mut self, max_bytes: u32) -> SdkResult<Vec<u8>> {
        self.inner.read(max_bytes)
    }

    pub fn read_exact(&mut self, mut size: usize) -> SdkResult<Vec<u8>> {
        let mut out = Vec::<u8>::with_capacity(size);
        while size > 0 {
            let chunk = self.inner.read(size.min(u32::MAX as usize) as u32)?;
            if chunk.is_empty() {
                return Err(SdkError::io("unexpected EOF while reading exact bytes"));
            }
            size = size.saturating_sub(chunk.len());
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    }

    pub fn read_to_end(&mut self, max_total: usize) -> SdkResult<Vec<u8>> {
        let mut out = Vec::<u8>::new();
        while out.len() < max_total {
            let remain = max_total - out.len();
            let chunk = self.inner.read(remain.min(u32::MAX as usize) as u32)?;
            if chunk.is_empty() {
                break;
            }
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    }

    pub fn seek(&mut self, offset: i64, whence: SeekWhence) -> SdkResult<u64> {
        self.inner.seek(offset, whence)
    }

    pub fn tell(&mut self) -> SdkResult<u64> {
        self.inner.tell()
    }

    pub fn size(&mut self) -> SdkResult<u64> {
        self.inner.size()
    }
}

impl<T: HostStreamHandle + ?Sized> DecoderInputStream for T {
    fn read(&mut self, max_bytes: u32) -> SdkResult<Vec<u8>> {
        HostStreamHandle::read(self, max_bytes)
    }

    fn seek(&mut self, offset: i64, whence: SeekWhence) -> SdkResult<u64> {
        HostStreamHandle::seek(self, offset, whence)
    }

    fn tell(&mut self) -> SdkResult<u64> {
        HostStreamHandle::tell(self)
    }

    fn size(&mut self) -> SdkResult<u64> {
        HostStreamHandle::size(self)
    }
}
