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

    pub(crate) fn size(&mut self) -> SdkResult<u64> {
        self.stream.size()
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

pub(crate) struct StreamMediaSource {
    reader: Mutex<DecoderInputReader>,
    byte_len: Option<u64>,
    seekable: bool,
}

impl StreamMediaSource {
    pub(crate) fn new(reader: DecoderInputReader, byte_len: Option<u64>, seekable: bool) -> Self {
        Self {
            reader: Mutex::new(reader),
            byte_len,
            seekable,
        }
    }
}

impl Read for StreamMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| std::io::Error::other("decoder input lock poisoned"))?;
        reader.read(buf)
    }
}

impl Seek for StreamMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        if !self.seekable {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "input stream is not seekable",
            ));
        }

        let mut reader = self
            .reader
            .lock()
            .map_err(|_| std::io::Error::other("decoder input lock poisoned"))?;
        reader.seek(pos)
    }
}

impl MediaSource for StreamMediaSource {
    fn is_seekable(&self) -> bool {
        self.seekable
    }

    fn byte_len(&self) -> Option<u64> {
        self.byte_len
    }
}
