//! The former native NCM reader, isolated in the plugin executable.
use anyhow::{Result, ensure};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

pub struct NcmSource {
    pub reader: ncmdump::Ncmdump<File>,
    pub info: ncmdump::NcmInfo,
    pub start: u64,
    pub length: u64,
}

impl NcmSource {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path)?;
        // Bound header allocations in ncmdump before handing it the reader.
        let size = file.metadata()?.len();
        let mut magic = [0; 10];
        file.read_exact(&mut magic)?;
        ensure!(&magic[..8] == b"CTENFDAM", "invalid NCM header");
        let key_len = read_u32(&mut file)?;
        ensure!((16..=4096).contains(&key_len), "invalid NCM key length");
        file.seek(SeekFrom::Current(i64::from(key_len)))?;
        let info_len = read_u32(&mut file)?;
        ensure!(
            (22..=1024 * 1024).contains(&info_len),
            "invalid NCM metadata length"
        );
        file.seek(SeekFrom::Current(i64::from(info_len) + 5))?;
        let cover_len = read_u32(&mut file)?;
        let image_len = read_u32(&mut file)?;
        ensure!(image_len <= cover_len, "invalid NCM cover length");
        ensure!(
            file.stream_position()? + u64::from(cover_len) < size,
            "missing NCM audio"
        );
        file.rewind()?;
        let mut reader = ncmdump::Ncmdump::from_reader(file)?;
        // Capture the logical payload offset before get_info moves the underlying
        // file into metadata. ncmdump's base uses image length, not cover length.
        let payload_start = reader.stream_position()?;
        let info = reader.get_info()?;
        ensure!(
            matches!(info.format.as_str(), "mp3" | "flac"),
            "unsupported NCM payload"
        );
        let end = reader.seek(SeekFrom::End(0))?;
        reader.seek(SeekFrom::Start(payload_start))?;
        let mut start = payload_start;
        if info.format == "flac" {
            let mut prefix = Vec::new();
            loop {
                let previous = prefix.len();
                let length = ((end - payload_start) as usize)
                    .min(previous + 16 * 1024)
                    .min(1024 * 1024);
                ensure!(length > previous, "NCM payload has no FLAC STREAMINFO");
                prefix.resize(length, 0);
                reader.read_exact(&mut prefix[previous..])?;
                if let Some(offset) = prefix
                    .windows(8)
                    .position(|v| &v[..4] == b"fLaC" && v[4] & 0x7f == 0 && v[5..8] == [0, 0, 34])
                {
                    start += offset as u64;
                    break;
                }
            }
        }
        reader.seek(SeekFrom::Start(start))?;
        Ok(Self {
            reader,
            info,
            start,
            length: end - start,
        })
    }
}

fn read_u32(file: &mut File) -> Result<u32> {
    let mut bytes = [0; 4];
    file.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
