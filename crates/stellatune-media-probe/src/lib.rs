//! Bounded, read-only technical audio properties; independent of tags and playback.
pub mod formats;
mod pcm;
pub mod slice;
#[cfg(test)]
mod tests;

use lofty::{
    config::ParseOptions,
    file::{AudioFile, FileType},
    probe::Probe,
};
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::{
    Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
pub mod types;
pub use types::{AudioProperties, BitrateInfo, BitrateKind, BitrateMode, ProbeResult, ProbeStatus};

pub const PROBE_VERSION: i64 = 1;
pub const READ_LIMIT: u64 = 8 * 1024 * 1024;
pub const TIME_LIMIT: Duration = Duration::from_secs(2);

/// Counts actual reads, including rereads. Seek skips do not consume the byte budget.
pub struct BudgetReader<'a, R> {
    inner: R,
    cancel: &'a AtomicBool,
    deadline: Instant,
    remaining: u64,
    failure: Option<ProbeStatus>,
}
impl<'a, R> BudgetReader<'a, R> {
    pub fn new(inner: R, cancel: &'a AtomicBool) -> Self {
        Self {
            inner,
            cancel,
            deadline: Instant::now() + TIME_LIMIT,
            remaining: READ_LIMIT,
            failure: None,
        }
    }
    fn check(&mut self) -> io::Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            self.failure = Some(ProbeStatus::Cancelled);
        } else if Instant::now() >= self.deadline {
            self.failure = Some(ProbeStatus::BudgetExceeded);
        }
        if self.failure.is_some() {
            return Err(io::Error::other("audio property probe stopped"));
        }
        Ok(())
    }
}
impl<R: Read> Read for BudgetReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.check()?;
        if buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            self.failure = Some(ProbeStatus::BudgetExceeded);
            return Err(io::Error::other("audio property read budget exceeded"));
        }
        let len = buf.len().min(self.remaining as usize);
        let n = self
            .inner
            .read(&mut buf[..len])
            .inspect_err(|_| self.failure = Some(ProbeStatus::IoError))?;
        self.remaining -= n as u64;
        self.check()?;
        Ok(n)
    }
}
impl<R: Seek> Seek for BudgetReader<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.check()?;
        let pos = self
            .inner
            .seek(pos)
            .inspect_err(|_| self.failure = Some(ProbeStatus::IoError))?;
        self.check()?;
        Ok(pos)
    }
}

static ACTIVE: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        *ACTIVE.0.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        ACTIVE.1.notify_one();
    }
}
/// No full-stream decoding or fallback scan. Deadline is cooperative: a blocked OS read
/// cannot be interrupted, but its result is discarded when it returns after the deadline.
pub fn probe<R: Read + Seek>(
    source: R,
    format_hint: Option<&str>,
    cancel: &AtomicBool,
) -> ProbeResult {
    let mut reader = BudgetReader::new(source, cancel);
    let permit = {
        let mut active = ACTIVE.0.lock().unwrap_or_else(|e| e.into_inner());
        while *active >= 2 {
            if reader.check().is_err() {
                return ProbeResult {
                    properties: None,
                    status: reader.failure.unwrap(),
                    bytes_read: 0,
                };
            }
            active = ACTIVE
                .1
                .wait_timeout(active, Duration::from_millis(20))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        *active += 1;
        Permit
    };
    let result = read_properties(&mut reader, format_hint);
    let _ = reader.check();
    drop(permit);
    let status = reader.failure.unwrap_or(match &result {
        Ok(Some(_)) => ProbeStatus::Ready,
        Ok(None) => ProbeStatus::Unsupported,
        Err(_) => ProbeStatus::Invalid,
    });
    ProbeResult {
        properties: if status == ProbeStatus::Ready {
            result.ok().flatten()
        } else {
            None
        },
        status,
        bytes_read: READ_LIMIT - reader.remaining,
    }
}

fn read_properties<R: Read + Seek>(
    reader: &mut R,
    format_hint: Option<&str>,
) -> Result<Option<AudioProperties>, Box<dyn std::error::Error>> {
    if let Some(pcm) = pcm::read(reader)? {
        return Ok(Some(pcm));
    }
    reader.seek(SeekFrom::Start(0))?;
    // Detect actual bytes. An extension cannot establish the codec in an Ogg/MP4 container.
    let options = ParseOptions::new()
        .read_properties(true)
        .read_tags(false)
        .read_cover_art(false)
        .implicit_conversions(false);
    let mut probe = Probe::new(reader).options(options).guess_file_type()?;
    // Hints only choose a parser when sniffing is inconclusive; that parser still
    // validates the real stream. Never infer codec or bitrate from the extension.
    if probe.file_type().is_none()
        && let Some(ty) = format_hint.and_then(FileType::from_ext)
    {
        probe = probe.set_file_type(ty);
    }
    if probe.file_type().is_none() {
        return Ok(None);
    }
    let ty = probe.file_type().unwrap();
    let (p, explicit_codec): (lofty::properties::FileProperties, Option<&str>) =
        if ty == FileType::Mp4 {
            let reader = probe.into_inner();
            reader.rewind()?;
            let file = lofty::mp4::Mp4File::read_from(reader, options)?;
            let codec = match file.properties().codec() {
                Some(lofty::mp4::Mp4Codec::AAC) => Some("aac"),
                Some(lofty::mp4::Mp4Codec::ALAC) => Some("alac"),
                Some(lofty::mp4::Mp4Codec::MP3) => Some("mp3"),
                Some(lofty::mp4::Mp4Codec::FLAC) => Some("flac"),
                _ => None,
            };
            (file.properties().clone().into(), codec)
        } else if ty == FileType::Mpeg {
            let reader = probe.into_inner();
            reader.rewind()?;
            let file = lofty::mpeg::MpegFile::read_from(reader, options)?;
            let codec = match file.properties().layer() {
                lofty::mpeg::Layer::Layer1 => "mp1",
                lofty::mpeg::Layer::Layer2 => "mp2",
                lofty::mpeg::Layer::Layer3 => "mp3",
            };
            ((*file.properties()).into(), Some(codec))
        } else {
            (probe.read()?.properties().clone(), None)
        };
    let (format, codec) = match ty {
        FileType::Mpeg => (explicit_codec.unwrap_or("MPEG"), explicit_codec),
        FileType::Aac => ("AAC", Some("aac")),
        FileType::Mp4 => ("M4A", explicit_codec),
        FileType::Flac => ("FLAC", Some("flac")),
        FileType::Vorbis => ("Ogg", Some("vorbis")),
        FileType::Opus => ("Ogg", Some("opus")),
        FileType::Speex => ("Ogg", Some("speex")),
        FileType::Ape => ("APE", Some("ape")),
        FileType::WavPack => ("WavPack", Some("wavpack")),
        FileType::Wav => ("WAV", None),
        FileType::Aiff => ("AIFF", None),
        FileType::Mpc => ("MPC", Some("mpc")),
        _ => return Ok(None),
    };
    let bitrate = p
        .audio_bitrate()
        .filter(|v| *v > 0)
        .and_then(|n| n.checked_mul(1000))
        .map(|bps| BitrateInfo {
            bps,
            kind: BitrateKind::Average,
            estimated: matches!(ty, FileType::Mpeg | FileType::Aac),
            mode: None,
        });
    Ok(Some(AudioProperties {
        format: Some(format.to_ascii_uppercase()),
        codec: codec.map(str::to_owned),
        sample_rate: p.sample_rate().filter(|n| *n > 0),
        bits_per_sample: p.bit_depth().filter(|n| *n > 0).map(u32::from),
        channels: p.channels().filter(|n| *n > 0).map(u32::from),
        floating_point: false,
        bitrate,
    }))
}
