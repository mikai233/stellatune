use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;

use symphonia::core::io::MediaSource;

pub(crate) struct NcmMediaSource {
    ncm: Mutex<ncmdump::Ncmdump<std::io::Cursor<Vec<u8>>>>,
    start: u64,
    len: u64,
}

impl NcmMediaSource {
    pub(crate) fn new(
        ncm: ncmdump::Ncmdump<std::io::Cursor<Vec<u8>>>,
        start: u64,
        len: u64,
    ) -> Self {
        Self {
            ncm: Mutex::new(ncm),
            start,
            len,
        }
    }
}

impl Read for NcmMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut ncm = self
            .ncm
            .lock()
            .map_err(|_| std::io::Error::other("ncm io lock poisoned"))?;
        Read::read(&mut *ncm, buf)
    }
}

impl Seek for NcmMediaSource {
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
                let end = len as i64 + n;
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

impl MediaSource for NcmMediaSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}
