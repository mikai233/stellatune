//! PCM storage geometry, not a bitrate frame scanner. Lofty's generic properties
//! discard block alignment, which matters for e.g. WAVE valid 24 bits in 32 bits.
use crate::{AudioProperties, BitrateInfo, BitrateKind, BitrateMode};
use std::io::{self, Read, Seek, SeekFrom};
fn le16(b: &[u8]) -> u32 {
    u16::from_le_bytes(b[..2].try_into().unwrap()) as u32
}
fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes(b[..4].try_into().unwrap())
}
fn be16(b: &[u8]) -> u32 {
    u16::from_be_bytes(b[..2].try_into().unwrap()) as u32
}
fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes(b[..4].try_into().unwrap())
}
fn properties(
    format: &str,
    rate: u32,
    channels: u32,
    bits: u32,
    frame_bytes: u32,
    float: bool,
) -> Option<AudioProperties> {
    if rate == 0
        || channels == 0
        || bits == 0
        || bits > 64
        || frame_bytes.checked_mul(8)? < channels.checked_mul(bits)?
    {
        return None;
    }
    Some(AudioProperties {
        format: Some(format.into()),
        codec: Some(if float { "pcm_float" } else { "pcm" }.into()),
        sample_rate: Some(rate),
        bits_per_sample: Some(bits),
        channels: Some(channels),
        floating_point: float,
        bitrate: Some(BitrateInfo {
            bps: rate.checked_mul(frame_bytes)?.checked_mul(8)?,
            kind: BitrateKind::Fixed,
            estimated: false,
            mode: Some(BitrateMode::Cbr),
        }),
    })
}
pub(super) fn read<R: Read + Seek>(r: &mut R) -> io::Result<Option<AudioProperties>> {
    r.seek(SeekFrom::Start(0))?;
    let mut head = [0; 12];
    if let Err(e) = r.read_exact(&mut head) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e);
    }
    let wav = &head[..4] == b"RIFF" && &head[8..] == b"WAVE";
    let aiff = &head[..4] == b"FORM" && matches!(&head[8..], b"AIFF" | b"AIFC");
    let caf = &head[..4] == b"caff";
    if !wav && !aiff && !caf {
        return Ok(None);
    }
    let end = r.seek(SeekFrom::End(0))?;
    let mut pos: u64 = if caf { 8 } else { 12 };
    while pos
        .checked_add(if caf { 12 } else { 8 })
        .is_some_and(|n| n <= end)
    {
        r.seek(SeekFrom::Start(pos))?;
        let mut chunk = [0; 12];
        let header = if caf { 12 } else { 8 };
        r.read_exact(&mut chunk[..header])?;
        let len = if caf {
            u64::from_be_bytes(chunk[4..12].try_into().unwrap())
        } else if wav {
            le32(&chunk[4..]) as u64
        } else {
            be32(&chunk[4..]) as u64
        };
        let data = pos + header as u64;
        if len > end.saturating_sub(data) {
            return Ok(None);
        }
        let mut b = [0; 40];
        if wav && &chunk[..4] == b"fmt " && len >= 16 {
            r.read_exact(&mut b[..len.min(40) as usize])?;
            let mut code = le16(&b);
            let mut bits = le16(&b[14..]);
            if code == 0xfffe {
                if len < 40
                    || le16(&b[16..]) < 22
                    || b[26..40] != [0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113]
                {
                    return Ok(None);
                }
                code = le16(&b[24..]);
                let valid = le16(&b[18..]);
                if valid > bits {
                    return Ok(None);
                }
                if valid > 0 {
                    bits = valid;
                }
            }
            return Ok(if matches!(code, 1 | 3) {
                properties(
                    "WAV",
                    le32(&b[4..]),
                    le16(&b[2..]),
                    bits,
                    le16(&b[12..]),
                    code == 3,
                )
            } else {
                None
            });
        }
        if aiff && &chunk[..4] == b"COMM" && len >= 18 {
            r.read_exact(&mut b[..len.min(22) as usize])?;
            let compressed = &head[8..] == b"AIFC";
            if compressed
                && (len < 22
                    || !matches!(
                        &b[18..22],
                        b"NONE" | b"twos" | b"sowt" | b"fl32" | b"FL32" | b"fl64" | b"FL64"
                    ))
            {
                return Ok(None);
            }
            let exp = be16(&b[8..]);
            let mantissa = u64::from_be_bytes(b[10..18].try_into().unwrap());
            let rate = (mantissa as f64) * 2f64.powi(exp as i32 - 16383 - 63);
            if exp & 0x8000 != 0
                || !rate.is_finite()
                || rate.fract() != 0.0
                || rate <= 0.0
                || rate > u32::MAX as f64
            {
                return Ok(None);
            }
            let bits = be16(&b[6..]);
            let channels = be16(&b);
            return Ok(properties(
                "AIFF",
                rate as u32,
                channels,
                bits,
                bits.div_ceil(8).saturating_mul(channels),
                compressed && matches!(&b[18..22], b"fl32" | b"FL32" | b"fl64" | b"FL64"),
            ));
        }
        if caf && &chunk[..4] == b"desc" && len >= 32 {
            r.read_exact(&mut b[..32])?;
            if &b[8..12] != b"lpcm" {
                return Ok(None);
            }
            let rate = f64::from_be_bytes(b[..8].try_into().unwrap());
            let packet_bytes = be32(&b[16..]);
            let packet_frames = be32(&b[20..]);
            if !rate.is_finite()
                || rate <= 0.0
                || rate > u32::MAX as f64
                || rate.fract() != 0.0
                || packet_frames == 0
                || !packet_bytes.is_multiple_of(packet_frames)
            {
                return Ok(None);
            }
            return Ok(properties(
                "CAF",
                rate as u32,
                be32(&b[24..]),
                be32(&b[28..]),
                packet_bytes / packet_frames,
                be32(&b[12..]) & 1 != 0,
            ));
        }
        pos = match data
            .checked_add(len)
            .and_then(|n| n.checked_add(if caf { 0 } else { len % 2 }))
        {
            Some(n) if n > pos => n,
            _ => return Ok(None),
        };
    }
    Ok(None)
}
