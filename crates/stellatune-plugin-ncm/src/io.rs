use std::io::{Read, Seek, SeekFrom};

use symphonia::core::io::MediaSource;

pub(crate) struct NcmMediaSource {
    ncm: ncmdump::Ncmdump<stellatune_plugin_sdk::HostIo>,
    start: u64,
    len: u64,
}

impl NcmMediaSource {
    pub(crate) fn new(
        ncm: ncmdump::Ncmdump<stellatune_plugin_sdk::HostIo>,
        start: u64,
        len: u64,
    ) -> Self {
        Self { ncm, start, len }
    }
}

impl Read for NcmMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Read::read(&mut self.ncm, buf)
    }
}

impl Seek for NcmMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let abs = match pos {
            SeekFrom::Start(n) => self.ncm.seek(SeekFrom::Start(self.start.saturating_add(n))),
            SeekFrom::Current(n) => self.ncm.seek(SeekFrom::Current(n)),
            SeekFrom::End(n) => {
                let end = self.len as i64 + n;
                if end < 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "seek before start",
                    ));
                }
                self.ncm
                    .seek(SeekFrom::Start(self.start.saturating_add(end as u64)))
            }
        }?;
        Ok(abs.saturating_sub(self.start))
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
