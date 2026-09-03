use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Mutex;

use stellatune_audio_core::{GaplessTrimSpec, PcmFormat};
use symphonia::core::io::MediaSource;

use crate::builtin_decoder::BuiltinDecoder;

const NCM_MAGIC: &[u8; 8] = b"CTENFDAM";

/// Native Rust decoder for NetEase `.ncm` containers.
///
/// The encrypted container is unwrapped in-process and its ordinary MP3/FLAC
/// payload is handed to the same Symphonia decoder used by builtin sources.
pub struct NcmDecoder {
    decoder: BuiltinDecoder,
}

impl NcmDecoder {
    pub fn open(path: &str) -> Result<Self, String> {
        let extension = Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !extension.eq_ignore_ascii_case("ncm") {
            return Err("native NCM decoder requires a .ncm file".to_string());
        }
        let mut file =
            File::open(path).map_err(|error| format!("open NCM file failed: {error}"))?;
        let mut magic = [0_u8; 8];
        file.read_exact(&mut magic)
            .map_err(|error| format!("read NCM magic failed: {error}"))?;
        if &magic != NCM_MAGIC {
            return Err("invalid NCM magic header".to_string());
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("reset NCM file failed: {error}"))?;

        let mut ncm = ncmdump::Ncmdump::from_reader(file)
            .map_err(|error| format!("parse NCM container failed: {error}"))?;
        // ncmdump positions the underlying reader at the encrypted audio after
        // parsing the cover frame, but initializes its logical cursor to zero.
        // Capture that real payload offset before `get_info`, which temporarily
        // seeks into metadata located before the payload base.
        let payload_start = ncm
            .stream_position()
            .map_err(|error| format!("locate NCM payload failed: {error}"))?;
        let info = ncm
            .get_info()
            .map_err(|error| format!("read NCM metadata failed: {error}"))?;
        let payload_end = ncm
            .seek(SeekFrom::End(0))
            .map_err(|error| format!("measure NCM payload failed: {error}"))?;
        ncm.seek(SeekFrom::Start(payload_start))
            .map_err(|error| format!("reset decrypted NCM payload failed: {error}"))?;

        let format = info.format.trim().to_ascii_lowercase();
        let start = if format == "flac" {
            find_flac_streaminfo_start(&mut ncm, payload_start)?
        } else {
            payload_start
        };
        ncm.seek(SeekFrom::Start(start))
            .map_err(|error| format!("seek decrypted NCM payload failed: {error}"))?;
        let length = payload_end.saturating_sub(start);
        let source = NcmMediaSource::new(ncm, start, length);
        let decoder = BuiltinDecoder::open_source(Box::new(source), format.as_str())?;
        Ok(Self { decoder })
    }

    pub fn spec(&self) -> PcmFormat {
        self.decoder.spec()
    }

    pub fn duration_ms_hint(&self) -> Option<u64> {
        self.decoder.effective_duration_ms_hint()
    }

    pub fn gapless_trim_spec(&self) -> Option<GaplessTrimSpec> {
        self.decoder.gapless_trim_spec()
    }

    pub fn seek_ms(&mut self, position_ms: u64) -> Result<(), String> {
        self.decoder.seek_ms(position_ms)
    }

    pub fn next_block(&mut self, frames: usize) -> Result<Option<Vec<f32>>, String> {
        self.decoder.next_block(frames)
    }
}

struct NcmMediaSource<R: Read> {
    ncm: Mutex<ncmdump::Ncmdump<R>>,
    start: u64,
    length: u64,
    position: u64,
}

impl<R> NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    fn new(ncm: ncmdump::Ncmdump<R>, start: u64, length: u64) -> Self {
        Self {
            ncm: Mutex::new(ncm),
            start,
            length,
            position: 0,
        }
    }
}

impl<R> Read for NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.length.saturating_sub(self.position);
        if remaining == 0 || buffer.is_empty() {
            return Ok(0);
        }
        let limit = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = self
            .ncm
            .lock()
            .map_err(|_| std::io::Error::other("NCM reader lock poisoned"))?
            .read(&mut buffer[..limit])?;
        self.position = self.position.saturating_add(read as u64);
        Ok(read)
    }
}

impl<R> Seek for NcmMediaSource<R>
where
    R: Read + Seek + Send + 'static,
{
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let mut ncm = self
            .ncm
            .lock()
            .map_err(|_| std::io::Error::other("NCM reader lock poisoned"))?;
        let target = match position {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.length) + i128::from(offset),
        };
        if target < 0 || target > i128::from(self.length) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "NCM seek is outside the decrypted payload",
            ));
        }
        let target = target as u64;
        let absolute = self.start.checked_add(target).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "NCM seek position overflowed",
            )
        })?;
        ncm.seek(SeekFrom::Start(absolute))?;
        self.position = target;
        Ok(target)
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
        Some(self.length)
    }
}

fn find_flac_streaminfo_start<R>(
    ncm: &mut ncmdump::Ncmdump<R>,
    payload_start: u64,
) -> Result<u64, String>
where
    R: Read + Seek,
{
    const MAX_SCAN_BYTES: u64 = 1024 * 1024;
    let original = ncm
        .stream_position()
        .map_err(|error| format!("locate FLAC payload failed: {error}"))?;
    ncm.seek(SeekFrom::Start(payload_start))
        .map_err(|error| format!("reset FLAC scan failed: {error}"))?;
    let mut offset = payload_start;
    let mut carry = Vec::new();
    let mut chunk = vec![0_u8; 16 * 1024];
    while offset.saturating_sub(payload_start) < MAX_SCAN_BYTES {
        let read = ncm
            .read(&mut chunk)
            .map_err(|error| format!("scan FLAC payload failed: {error}"))?;
        if read == 0 {
            break;
        }
        let mut window = carry;
        window.extend_from_slice(&chunk[..read]);
        if window.len() >= 8 {
            for index in 0..=window.len() - 8 {
                if &window[index..index + 4] == b"fLaC"
                    && window[index + 4] & 0x7f == 0
                    && window[index + 5..index + 8] == [0, 0, 0x22]
                {
                    let found = offset
                        .saturating_sub(window.len().saturating_sub(read) as u64)
                        .saturating_add(index as u64);
                    ncm.seek(SeekFrom::Start(original)).ok();
                    return Ok(found);
                }
            }
        }
        offset = offset.saturating_add(read as u64);
        let keep = window.len().min(7);
        carry = window[window.len() - keep..].to_vec();
    }
    ncm.seek(SeekFrom::Start(original)).ok();
    Ok(payload_start)
}

#[cfg(test)]
mod tests {
    use super::NCM_MAGIC;

    #[test]
    fn ncm_magic_is_the_expected_reversed_header() {
        assert_eq!(NCM_MAGIC, b"CTENFDAM");
    }
}
