use core::ffi::c_void;
use std::io::{self, Read, Seek, SeekFrom};

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    ST_DECODER_INFO_FLAG_HAS_DURATION, ST_DECODER_INFO_FLAG_SEEKABLE, ST_LAYOUT_STEREO,
    ST_OUTPUT_NEGOTIATE_EXACT, SdkError, SdkResult, StAudioSpec, StDecoderInfo, StIoVTable,
    StOutputSinkNegotiatedSpec, StSeekWhence,
};

pub trait Dsp: Send + 'static {
    type Config: Serialize + DeserializeOwned;

    fn set_config(&mut self, _config: &Self::Config) -> SdkResult<()> {
        Ok(())
    }

    fn process_interleaved_f32_in_place(&mut self, samples: &mut [f32], frames: u32);
}

pub trait DspDescriptor: Dsp {
    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    fn default_config() -> Self::Config;

    /// Bitmask of supported channel layouts (ST_LAYOUT_* flags).
    /// Default: ST_LAYOUT_STEREO (stereo only).
    const SUPPORTED_LAYOUTS: u32 = ST_LAYOUT_STEREO;

    /// Output channel count if this DSP changes channel count.
    /// Return 0 to preserve input channel count (passthrough).
    const OUTPUT_CHANNELS: u16 = 0;

    fn create(spec: StAudioSpec, config: Self::Config) -> SdkResult<Self>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderInfo {
    pub spec: StAudioSpec,
    pub duration_ms: Option<u64>,
    pub seekable: bool,
}

impl DecoderInfo {
    pub fn to_ffi(self) -> StDecoderInfo {
        let mut flags = 0u32;
        if self.seekable {
            flags |= ST_DECODER_INFO_FLAG_SEEKABLE;
        }
        let mut duration_ms = 0u64;
        if let Some(d) = self.duration_ms {
            flags |= ST_DECODER_INFO_FLAG_HAS_DURATION;
            duration_ms = d;
        }
        StDecoderInfo {
            spec: self.spec,
            duration_ms,
            flags,
            reserved: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct HostIo {
    vtable: *const StIoVTable,
    handle: *mut c_void,
}

unsafe impl Send for HostIo {}
// Raw pointers make this not auto-Sync. StellaTune treats the IO vtable as immutable, and the
// host must ensure any IO handle is thread-safe if it is accessed from multiple threads.
unsafe impl Sync for HostIo {}

impl HostIo {
    /// # Safety
    ///
    /// The caller must ensure that `vtable` and `handle` are valid and remain valid for the
    /// lifetime of the returned `HostIo`.
    pub unsafe fn from_raw(vtable: *const StIoVTable, handle: *mut c_void) -> Self {
        Self { vtable, handle }
    }

    pub fn is_seekable(self) -> bool {
        if self.vtable.is_null() {
            return false;
        }
        unsafe { (*self.vtable).seek.is_some() }
    }

    pub fn size(self) -> io::Result<u64> {
        if self.vtable.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "null io_vtable",
            ));
        }
        let Some(size) = (unsafe { (*self.vtable).size }) else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "size unsupported",
            ));
        };
        let mut out = 0u64;
        let st = (size)(self.handle, &mut out);
        if st.code != 0 {
            return Err(io::Error::other(format!("size failed (code={})", st.code)));
        }
        Ok(out)
    }
}

impl Read for HostIo {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.vtable.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "null io_vtable",
            ));
        }
        let mut out_read: usize = 0;
        let st = unsafe {
            ((*self.vtable).read)(self.handle, buf.as_mut_ptr(), buf.len(), &mut out_read)
        };
        if st.code != 0 {
            return Err(io::Error::other(format!("read failed (code={})", st.code)));
        }
        Ok(out_read)
    }
}

impl Seek for HostIo {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        if self.vtable.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "null io_vtable",
            ));
        }
        let Some(seek) = (unsafe { (*self.vtable).seek }) else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "seek unsupported",
            ));
        };
        let (offset, whence) = match pos {
            SeekFrom::Start(n) => (n as i64, StSeekWhence::Start),
            SeekFrom::Current(n) => (n, StSeekWhence::Current),
            SeekFrom::End(n) => (n, StSeekWhence::End),
        };
        let mut out_pos = 0u64;
        let st = (seek)(self.handle, offset, whence, &mut out_pos);
        if st.code != 0 {
            return Err(io::Error::other(format!("seek failed (code={})", st.code)));
        }
        Ok(out_pos)
    }
}

pub struct DecoderOpenArgs<'a> {
    pub path: &'a str,
    pub ext: &'a str,
    pub io: HostIo,
}

pub trait Decoder: Send + 'static {
    type Metadata: Serialize;

    fn info(&self) -> DecoderInfo;

    fn seek_ms(&mut self, _position_ms: u64) -> SdkResult<()> {
        Err(SdkError::msg("seek not supported"))
    }

    fn metadata(&self) -> Option<Self::Metadata> {
        None
    }

    /// Fill `out_interleaved` with up to `frames` frames.
    /// Returns `(frames_written, eof)`.
    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)>;
}

pub trait DecoderDescriptor: Decoder {
    const TYPE_ID: &'static str;
    const SUPPORTS_SEEK: bool = true;

    fn probe(_path_ext: &str, _header: &[u8]) -> u8 {
        0
    }

    fn open(args: DecoderOpenArgs<'_>) -> SdkResult<Self>
    where
        Self: Sized;
}

pub trait OutputSink: Send + 'static {
    /// Writes interleaved f32 samples and returns accepted frame count.
    fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> SdkResult<u32>;

    fn flush(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait OutputSinkDescriptor: OutputSink {
    type Config: Serialize + DeserializeOwned;
    type Target: Serialize + DeserializeOwned;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    fn default_config() -> Self::Config;

    fn list_targets(config: &Self::Config) -> SdkResult<Vec<Self::Target>>;

    fn negotiate_spec(
        desired_spec: StAudioSpec,
        _config: &Self::Config,
        _target: &Self::Target,
    ) -> SdkResult<StOutputSinkNegotiatedSpec> {
        Ok(StOutputSinkNegotiatedSpec {
            spec: StAudioSpec {
                sample_rate: desired_spec.sample_rate.max(1),
                channels: desired_spec.channels.max(1),
                reserved: 0,
            },
            preferred_chunk_frames: 0,
            flags: ST_OUTPUT_NEGOTIATE_EXACT,
            reserved: 0,
        })
    }

    fn open(spec: StAudioSpec, config: &Self::Config, target: &Self::Target) -> SdkResult<Self>
    where
        Self: Sized;
}

pub trait SourceStream: Send + 'static {
    const SUPPORTS_SEEK: bool = false;
    const SUPPORTS_TELL: bool = false;
    const SUPPORTS_SIZE: bool = false;

    fn read(&mut self, out: &mut [u8]) -> SdkResult<usize>;

    fn seek(&mut self, _offset: i64, _whence: StSeekWhence) -> SdkResult<u64> {
        Err(SdkError::msg("seek unsupported"))
    }

    fn tell(&mut self) -> SdkResult<u64> {
        Err(SdkError::msg("tell unsupported"))
    }

    fn size(&mut self) -> SdkResult<u64> {
        Err(SdkError::msg("size unsupported"))
    }
}

pub struct SourceOpenResult<S: SourceStream, M> {
    pub stream: S,
    pub track_meta: Option<M>,
}

impl<S: SourceStream, M> SourceOpenResult<S, M> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            track_meta: None,
        }
    }

    pub fn with_track_meta(mut self, track_meta: M) -> Self {
        self.track_meta = Some(track_meta);
        self
    }
}

pub trait SourceCatalogDescriptor: Send + Sync + 'static {
    type Stream: SourceStream;
    type Config: Serialize + DeserializeOwned;
    type ListRequest: DeserializeOwned;
    type ListItem: Serialize;
    type Track: DeserializeOwned;
    type TrackMeta: Serialize;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    fn default_config() -> Self::Config;

    fn list_items(
        config: &Self::Config,
        request: &Self::ListRequest,
    ) -> SdkResult<Vec<Self::ListItem>>;

    fn open_stream(
        config: &Self::Config,
        track: &Self::Track,
    ) -> SdkResult<SourceOpenResult<Self::Stream, Self::TrackMeta>>;
}

#[doc(hidden)]
pub struct DspBox<T: Dsp> {
    pub inner: T,
    pub channels: u16,
}

#[doc(hidden)]
pub struct OutputSinkBox<T: OutputSink> {
    pub inner: T,
}

#[doc(hidden)]
pub struct SourceStreamBox<T: SourceStream> {
    pub inner: T,
}

#[doc(hidden)]
#[allow(dead_code)]
pub struct DecoderBox<T: Decoder> {
    pub inner: T,
    pub channels: u16,
}
