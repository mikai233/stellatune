//! A zero-based view of an audio payload inside another seekable resource.
use std::io::{self, Read, Seek, SeekFrom};
pub struct AudioSlice<R> {
    inner: R,
    start: u64,
    length: u64,
    position: u64,
}
impl<R: Seek> AudioSlice<R> {
    pub fn new(mut inner: R, start: u64, length: u64) -> io::Result<Self> {
        start
            .checked_add(length)
            .ok_or_else(|| io::Error::other("audio slice overflow"))?;
        inner.seek(SeekFrom::Start(start))?;
        Ok(Self {
            inner,
            start,
            length,
            position: 0,
        })
    }
}
impl<R: Read> Read for AudioSlice<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = (self.length - self.position).min(buf.len() as u64) as usize;
        if len == 0 {
            return Ok(0);
        }
        let n = self.inner.read(&mut buf[..len])?;
        self.position += n as u64;
        Ok(n)
    }
}
impl<R: Seek> Seek for AudioSlice<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let pos = match pos {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => i128::from(self.length) + i128::from(n),
        };
        if pos < 0 || pos > i128::from(self.length) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek outside audio payload",
            ));
        }
        let pos = pos as u64;
        self.inner.seek(SeekFrom::Start(self.start + pos))?;
        self.position = pos;
        Ok(pos)
    }
}
