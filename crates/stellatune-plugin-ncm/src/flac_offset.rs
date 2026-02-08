use std::io::{Read, Seek, SeekFrom};

fn parse_id3v2_total_size(header10: &[u8; 10]) -> Option<u64> {
    if &header10[..3] != b"ID3" {
        return None;
    }
    // ID3v2 flags are in header[5]; lower nibble must be zero.
    if (header10[5] & 0x0f) != 0 {
        return None;
    }
    let size: u64 = (((header10[6] & 0x7f) as u64) << 21)
        | (((header10[7] & 0x7f) as u64) << 14)
        | (((header10[8] & 0x7f) as u64) << 7)
        | ((header10[9] & 0x7f) as u64);
    let has_footer = (header10[5] & 0x10) != 0;
    Some(
        10u64
            .saturating_add(size)
            .saturating_add(if has_footer { 10 } else { 0 }),
    )
}

pub(crate) fn find_flac_streaminfo_start<R>(ncm: &mut ncmdump::Ncmdump<R>) -> Result<u64, String>
where
    R: Read + Seek,
{
    // Don't scan unbounded amounts of data: this is just to skip leading junk (e.g. ID3 or
    // container garbage). If FLAC doesn't appear early, treat the stream as-is.
    const MAX_SCAN: u64 = 1024 * 1024;

    fn is_flac_streaminfo_at(buf: &[u8]) -> bool {
        if buf.len() < 8 {
            return false;
        }
        if &buf[..4] != b"fLaC" {
            return false;
        }
        // First metadata block is STREAMINFO (type 0), size is always 34 bytes.
        let block_header = buf[4];
        let block_type = block_header & 0x7f;
        if block_type != 0 {
            return false;
        }
        buf[5..8] == [0x00, 0x00, 0x22]
    }

    let pos0 = ncm
        .stream_position()
        .map_err(|e| format!("tell failed: {e}"))?;
    ncm.seek(SeekFrom::Start(0))
        .map_err(|e| format!("seek start failed: {e}"))?;

    let mut head16 = [0u8; 16];
    let head_n = ncm
        .read(&mut head16)
        .map_err(|e| format!("read header failed: {e}"))?;
    ncm.seek(SeekFrom::Start(0))
        .map_err(|e| format!("seek start failed: {e}"))?;

    if head_n >= 8 && is_flac_streaminfo_at(&head16[..8]) {
        let _ = ncm.seek(SeekFrom::Start(pos0));
        return Ok(0);
    }

    if head_n >= 10 {
        let mut head10 = [0u8; 10];
        head10.copy_from_slice(&head16[..10]);
        if let Some(skip) = parse_id3v2_total_size(&head10) {
            if ncm.seek(SeekFrom::Start(skip)).is_ok() {
                let mut next8 = [0u8; 8];
                if ncm.read_exact(&mut next8).is_ok() && is_flac_streaminfo_at(&next8) {
                    let _ = ncm.seek(SeekFrom::Start(pos0));
                    return Ok(skip);
                }
            }
            let _ = ncm.seek(SeekFrom::Start(0));
        }
    }

    let mut scanned: u64 = 0;
    let mut carry: Vec<u8> = Vec::new(); // keep last 7 bytes for cross-chunk match
    let mut chunk = vec![0u8; 16 * 1024];
    while scanned < MAX_SCAN {
        let n = ncm
            .read(&mut chunk)
            .map_err(|e| format!("scan read failed: {e}"))?;
        if n == 0 {
            break;
        }

        let base_offset = scanned;
        scanned = scanned.saturating_add(n as u64);

        let mut window = Vec::with_capacity(carry.len() + n);
        window.extend_from_slice(&carry);
        window.extend_from_slice(&chunk[..n]);

        if window.len() >= 8 {
            for i in 0..=(window.len() - 8) {
                if &window[i..i + 4] == b"fLaC" && is_flac_streaminfo_at(&window[i..i + 8]) {
                    let window_start = base_offset.saturating_sub(carry.len() as u64);
                    let off = window_start.saturating_add(i as u64);
                    let _ = ncm.seek(SeekFrom::Start(pos0));
                    return Ok(off);
                }
            }
        }

        let keep = 7usize.min(window.len());
        carry.clear();
        carry.extend_from_slice(&window[window.len() - keep..]);
    }

    let _ = ncm.seek(SeekFrom::Start(pos0));
    Ok(0)
}
