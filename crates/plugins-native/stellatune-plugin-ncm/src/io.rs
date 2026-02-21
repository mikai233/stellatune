use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;

use stellatune_plugin_sdk::prelude::*;
use symphonia::core::io::MediaSource;

pub(crate) struct DecoderInputReader {
    stream: Box<dyn DecoderInputStream>,
}

impl DecoderInputReader {
    pub(crate) fn new(stream: Box<dyn DecoderInputStream>) -> Self {
        Self { stream }
    }
}

fn map_sdk_error(error: SdkError) -> std::io::Error {
    std::io::Error::other(error.to_string())
}

impl Read for DecoderInputReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let max_bytes = buf.len().min(u32::MAX as usize) as u32;
        let chunk = self.stream.read(max_bytes).map_err(map_sdk_error)?;
        let len = chunk.len().min(buf.len());
        buf[..len].copy_from_slice(&chunk[..len]);
        Ok(len)
    }
}

impl Seek for DecoderInputReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let (offset, whence) = match pos {
            SeekFrom::Start(offset) => {
                if offset > i64::MAX as u64 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "seek start offset exceeds i64 range",
                    ));
                }
                (offset as i64, SeekWhence::Start)
            },
            SeekFrom::Current(offset) => (offset, SeekWhence::Current),
            SeekFrom::End(offset) => (offset, SeekWhence::End),
        };
        self.stream.seek(offset, whence).map_err(map_sdk_error)
    }
}

pub(crate) struct NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    ncm: Mutex<ncmdump::Ncmdump<R>>,
    start: u64,
    len: Option<u64>,
}

impl<R> NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    pub(crate) fn new(ncm: ncmdump::Ncmdump<R>, start: u64, len: Option<u64>) -> Self {
        Self {
            ncm: Mutex::new(ncm),
            start,
            len,
        }
    }
}

impl<R> Read for NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut ncm = self
            .ncm
            .lock()
            .map_err(|_| std::io::Error::other("ncm io lock poisoned"))?;
        Read::read(&mut *ncm, buf)
    }
}

impl<R> Seek for NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let start = self.start;
        let len = self.len;
        let mut ncm = self
            .ncm
            .lock()
            .map_err(|_| std::io::Error::other("ncm io lock poisoned"))?;
        let abs = match pos {
            SeekFrom::Start(n) => ncm.seek(SeekFrom::Start(start.saturating_add(n))),
            SeekFrom::Current(n) => ncm.seek(SeekFrom::Current(n)),
            SeekFrom::End(n) => {
                let Some(total) = len else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "seek from end is not available for unknown length",
                    ));
                };
                let end = total as i64 + n;
                if end < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "seek before start",
                    ));
                }
                ncm.seek(SeekFrom::Start(start.saturating_add(end as u64)))
            },
        }?;
        Ok(abs.saturating_sub(start))
    }
}

impl<R> MediaSource for NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}
