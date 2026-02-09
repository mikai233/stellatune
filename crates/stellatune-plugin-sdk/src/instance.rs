use serde::{Serialize, de::DeserializeOwned};
use stellatune_plugin_api::StOutputSinkNegotiatedSpec;

use crate::{SdkError, SdkResult, StAudioSpec, StDecoderInfo, StIoVTable, StSeekWhence};

use super::update::ConfigUpdatable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderExtScoreRule {
    /// Lowercase extension without dot (e.g. "flac").
    /// "*" means wildcard fallback.
    pub ext: &'static str,
    /// Higher score wins for decoder ordering.
    pub score: u16,
}

pub trait DecoderInstance: Send + ConfigUpdatable + 'static {
    fn open(&mut self, _args: DecoderOpenArgsRef<'_>) -> SdkResult<()> {
        Err(SdkError::msg("decoder open is not implemented"))
    }

    fn get_info(&self) -> StDecoderInfo;

    fn get_metadata_json(&self) -> SdkResult<Option<String>> {
        Ok(None)
    }

    /// Fill `out_interleaved` with up to `frames` frames.
    /// Returns `(frames_written, eof)`.
    fn read_interleaved_f32(
        &mut self,
        frames: u32,
        out_interleaved: &mut [f32],
    ) -> SdkResult<(u32, bool)>;

    fn seek_ms(&mut self, _position_ms: u64) -> SdkResult<()> {
        Err(SdkError::msg("seek not supported"))
    }
}

#[derive(Clone, Copy)]
pub struct DecoderOpenIoRef {
    pub io_vtable: *const StIoVTable,
    pub io_handle: *mut core::ffi::c_void,
}

impl DecoderOpenIoRef {
    pub fn read(&mut self, out: &mut [u8]) -> SdkResult<usize> {
        if self.io_vtable.is_null() {
            return Err(SdkError::msg("decoder io_vtable is null"));
        }
        let mut out_read = 0usize;
        let status = unsafe {
            ((*self.io_vtable).read)(self.io_handle, out.as_mut_ptr(), out.len(), &mut out_read)
        };
        if status.code != 0 {
            return Err(SdkError::msg(format!(
                "decoder io read failed: code={}",
                status.code
            )));
        }
        Ok(out_read.min(out.len()))
    }

    pub fn seek(&mut self, offset: i64, whence: StSeekWhence) -> SdkResult<u64> {
        if self.io_vtable.is_null() {
            return Err(SdkError::msg("decoder io_vtable is null"));
        }
        let Some(seek_fn) = (unsafe { (*self.io_vtable).seek }) else {
            return Err(SdkError::msg("decoder io seek unsupported"));
        };
        let mut out_pos = 0u64;
        let status = seek_fn(self.io_handle, offset, whence, &mut out_pos);
        if status.code != 0 {
            return Err(SdkError::msg(format!(
                "decoder io seek failed: code={}",
                status.code
            )));
        }
        Ok(out_pos)
    }

    pub fn tell(&mut self) -> SdkResult<u64> {
        if self.io_vtable.is_null() {
            return Err(SdkError::msg("decoder io_vtable is null"));
        }
        let Some(tell_fn) = (unsafe { (*self.io_vtable).tell }) else {
            return Err(SdkError::msg("decoder io tell unsupported"));
        };
        let mut out_pos = 0u64;
        let status = tell_fn(self.io_handle, &mut out_pos);
        if status.code != 0 {
            return Err(SdkError::msg(format!(
                "decoder io tell failed: code={}",
                status.code
            )));
        }
        Ok(out_pos)
    }

    pub fn size(&mut self) -> SdkResult<u64> {
        if self.io_vtable.is_null() {
            return Err(SdkError::msg("decoder io_vtable is null"));
        }
        let Some(size_fn) = (unsafe { (*self.io_vtable).size }) else {
            return Err(SdkError::msg("decoder io size unsupported"));
        };
        let mut out_size = 0u64;
        let status = size_fn(self.io_handle, &mut out_size);
        if status.code != 0 {
            return Err(SdkError::msg(format!(
                "decoder io size failed: code={}",
                status.code
            )));
        }
        Ok(out_size)
    }
}

#[derive(Clone, Copy)]
pub struct DecoderOpenArgsRef<'a> {
    pub path_hint: &'a str,
    pub ext_hint: &'a str,
    pub io: DecoderOpenIoRef,
}

pub trait DecoderDescriptor {
    type Config: Serialize + DeserializeOwned;
    type Instance: DecoderInstance;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    const EXT_SCORE_RULES: &'static [DecoderExtScoreRule] = &[];
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

pub trait DspInstance: Send + ConfigUpdatable + 'static {
    /// Process interleaved f32 samples in place.
    fn process_interleaved_f32_in_place(&mut self, samples: &mut [f32], frames: u32);

    /// Bitmask of supported input layouts (ST_LAYOUT_* flags).
    fn supported_layouts(&self) -> u32;

    /// Output channels if DSP changes channel count. 0 means passthrough.
    fn output_channels(&self) -> u16 {
        0
    }
}

pub trait DspDescriptor {
    type Config: Serialize + DeserializeOwned;
    type Instance: DspInstance;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(spec: StAudioSpec, config: Self::Config) -> SdkResult<Self::Instance>;
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

pub struct SourceOpenResult<S: SourceStream> {
    pub stream: S,
    pub track_meta_json: Option<String>,
}

impl<S: SourceStream> SourceOpenResult<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            track_meta_json: None,
        }
    }

    pub fn with_track_meta_json(mut self, track_meta_json: impl Into<String>) -> Self {
        self.track_meta_json = Some(track_meta_json.into());
        self
    }
}

pub trait SourceCatalogInstance: Send + ConfigUpdatable + 'static {
    fn list_items_json(&mut self, request_json: &str) -> SdkResult<String>;

    fn open_stream_json(&mut self, track_json: &str) -> SdkResult<SourceOpenResult<Self::Stream>>;

    fn close_stream(&mut self, _stream: &mut Self::Stream) -> SdkResult<()> {
        Ok(())
    }

    type Stream: SourceStream;
}

pub trait SourceCatalogDescriptor {
    type Config: Serialize + DeserializeOwned;
    type Instance: SourceCatalogInstance;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

pub trait LyricsProviderInstance: Send + ConfigUpdatable + 'static {
    fn search_json(&mut self, query_json: &str) -> SdkResult<String>;
    fn fetch_json(&mut self, track_json: &str) -> SdkResult<String>;
}

pub trait LyricsProviderDescriptor {
    type Config: Serialize + DeserializeOwned;
    type Instance: LyricsProviderInstance;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

pub trait OutputSinkInstance: Send + ConfigUpdatable + 'static {
    fn list_targets_json(&mut self) -> SdkResult<String>;

    fn negotiate_spec_json(
        &mut self,
        target_json: &str,
        desired_spec: StAudioSpec,
    ) -> SdkResult<StOutputSinkNegotiatedSpec>;

    fn open_json(&mut self, target_json: &str, spec: StAudioSpec) -> SdkResult<()>;

    /// Writes interleaved f32 samples and returns accepted frame count.
    fn write_interleaved_f32(&mut self, channels: u16, samples: &[f32]) -> SdkResult<u32>;

    fn flush(&mut self) -> SdkResult<()> {
        Ok(())
    }

    fn close(&mut self) -> SdkResult<()> {
        Ok(())
    }
}

pub trait OutputSinkDescriptor {
    type Config: Serialize + DeserializeOwned;
    type Instance: OutputSinkInstance;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

#[doc(hidden)]
pub struct DecoderBox<T: DecoderInstance> {
    pub inner: T,
    pub channels: u16,
}

#[doc(hidden)]
pub struct DspBox<T: DspInstance> {
    pub inner: T,
    pub channels: u16,
}

#[doc(hidden)]
pub struct SourceCatalogBox<T: SourceCatalogInstance> {
    pub inner: T,
}

#[doc(hidden)]
pub struct SourceStreamBox<T: SourceStream> {
    pub inner: T,
}

#[doc(hidden)]
pub struct LyricsProviderBox<T: LyricsProviderInstance> {
    pub inner: T,
}

#[doc(hidden)]
pub struct OutputSinkBox<T: OutputSinkInstance> {
    pub inner: T,
}
