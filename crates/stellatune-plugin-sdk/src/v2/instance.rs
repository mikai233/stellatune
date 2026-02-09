use serde::{Serialize, de::DeserializeOwned};
use stellatune_plugin_api::v2::StOutputSinkNegotiatedSpecV2;

use crate::{SdkError, SdkResult, StAudioSpec, StDecoderInfoV1, StIoVTableV1, StSeekWhence};

use super::update::ConfigUpdatableV2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoderExtScoreRuleV2 {
    /// Lowercase extension without dot (e.g. "flac").
    /// "*" means wildcard fallback.
    pub ext: &'static str,
    /// Higher score wins for decoder ordering.
    pub score: u16,
}

pub trait DecoderInstanceV2: Send + ConfigUpdatableV2 + 'static {
    fn open(&mut self, _args: DecoderOpenArgsRefV2<'_>) -> SdkResult<()> {
        Err(SdkError::msg("decoder open is not implemented"))
    }

    fn get_info(&self) -> StDecoderInfoV1;

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
pub struct DecoderOpenIoRefV2 {
    pub io_vtable: *const StIoVTableV1,
    pub io_handle: *mut core::ffi::c_void,
}

impl DecoderOpenIoRefV2 {
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
pub struct DecoderOpenArgsRefV2<'a> {
    pub path_hint: &'a str,
    pub ext_hint: &'a str,
    pub io: DecoderOpenIoRefV2,
}

pub trait DecoderDescriptorV2 {
    type Config: Serialize + DeserializeOwned;
    type Instance: DecoderInstanceV2;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    const EXT_SCORE_RULES: &'static [DecoderExtScoreRuleV2] = &[];
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

pub trait DspInstanceV2: Send + ConfigUpdatableV2 + 'static {
    /// Process interleaved f32 samples in place.
    fn process_interleaved_f32_in_place(&mut self, samples: &mut [f32], frames: u32);

    /// Bitmask of supported input layouts (ST_LAYOUT_* flags).
    fn supported_layouts(&self) -> u32;

    /// Output channels if DSP changes channel count. 0 means passthrough.
    fn output_channels(&self) -> u16 {
        0
    }
}

pub trait DspDescriptorV2 {
    type Config: Serialize + DeserializeOwned;
    type Instance: DspInstanceV2;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(spec: StAudioSpec, config: Self::Config) -> SdkResult<Self::Instance>;
}

pub trait SourceStreamV2: Send + 'static {
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

pub struct SourceOpenResultV2<S: SourceStreamV2> {
    pub stream: S,
    pub track_meta_json: Option<String>,
}

impl<S: SourceStreamV2> SourceOpenResultV2<S> {
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

pub trait SourceCatalogInstanceV2: Send + ConfigUpdatableV2 + 'static {
    fn list_items_json(&mut self, request_json: &str) -> SdkResult<String>;

    fn open_stream_json(&mut self, track_json: &str)
    -> SdkResult<SourceOpenResultV2<Self::Stream>>;

    fn close_stream(&mut self, _stream: &mut Self::Stream) -> SdkResult<()> {
        Ok(())
    }

    type Stream: SourceStreamV2;
}

pub trait SourceCatalogDescriptorV2 {
    type Config: Serialize + DeserializeOwned;
    type Instance: SourceCatalogInstanceV2;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

pub trait LyricsProviderInstanceV2: Send + ConfigUpdatableV2 + 'static {
    fn search_json(&mut self, query_json: &str) -> SdkResult<String>;
    fn fetch_json(&mut self, track_json: &str) -> SdkResult<String>;
}

pub trait LyricsProviderDescriptorV2 {
    type Config: Serialize + DeserializeOwned;
    type Instance: LyricsProviderInstanceV2;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

pub trait OutputSinkInstanceV2: Send + ConfigUpdatableV2 + 'static {
    fn list_targets_json(&mut self) -> SdkResult<String>;

    fn negotiate_spec_json(
        &mut self,
        target_json: &str,
        desired_spec: StAudioSpec,
    ) -> SdkResult<StOutputSinkNegotiatedSpecV2>;

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

pub trait OutputSinkDescriptorV2 {
    type Config: Serialize + DeserializeOwned;
    type Instance: OutputSinkInstanceV2;

    const TYPE_ID: &'static str;
    const DISPLAY_NAME: &'static str;
    const CONFIG_SCHEMA_JSON: &'static str;
    const DEFAULT_CONFIG_JSON: &'static str = "{}";
    fn default_config() -> Self::Config;

    fn create(config: Self::Config) -> SdkResult<Self::Instance>;
}

#[doc(hidden)]
pub struct DecoderBoxV2<T: DecoderInstanceV2> {
    pub inner: T,
    pub channels: u16,
}

#[doc(hidden)]
pub struct DspBoxV2<T: DspInstanceV2> {
    pub inner: T,
    pub channels: u16,
}

#[doc(hidden)]
pub struct SourceCatalogBoxV2<T: SourceCatalogInstanceV2> {
    pub inner: T,
}

#[doc(hidden)]
pub struct SourceStreamBoxV2<T: SourceStreamV2> {
    pub inner: T,
}

#[doc(hidden)]
pub struct LyricsProviderBoxV2<T: LyricsProviderInstanceV2> {
    pub inner: T,
}

#[doc(hidden)]
pub struct OutputSinkBoxV2<T: OutputSinkInstanceV2> {
    pub inner: T,
}
