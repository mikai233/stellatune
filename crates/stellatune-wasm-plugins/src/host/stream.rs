use crate::error::Result;

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

pub trait HostStreamService: Send + Sync {
    fn open_uri(&self, uri: &str) -> Result<Box<dyn HostStreamHandle>>;
}
