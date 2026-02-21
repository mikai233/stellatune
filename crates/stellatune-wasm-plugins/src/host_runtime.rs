use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::AtomicU64;

pub mod runtime_service;

use crate::error::Error as WasmPluginError;
use crate::executor::plugin_instance::decoder::{DecoderPluginApi, WasmtimeDecoderPlugin};
use crate::executor::plugin_instance::dsp::{DspPluginApi, WasmtimeDspPlugin};
use crate::executor::plugin_instance::lyrics::{LyricsPluginApi, WasmtimeLyricsPlugin};
use crate::executor::plugin_instance::output_sink::{
    OutputSinkPluginApi, WasmtimeOutputSinkPlugin,
};
use crate::executor::plugin_instance::source::{
    RuntimeOpenedSourceStream, SourcePluginApi, WasmtimeSourcePlugin,
};

use crate::host::stream::{HostStreamHandle, open_local_file_stream};
use crate::runtime::model::{
    RuntimeAudioSpec, RuntimeDecoderSessionHandle, RuntimeDspProcessorHandle, RuntimeEncodedChunk,
    RuntimeMediaMetadata, RuntimeNegotiatedSpec, RuntimeOutputSinkStatus, RuntimePcmF32Chunk,
    RuntimeSourceStreamHandle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCapabilityKind {
    Decoder,
    Dsp,
    SourceCatalog,
    LyricsProvider,
    OutputSink,
}

#[derive(Debug, Clone)]
pub struct RuntimeCapabilityDescriptor {
    pub kind: RuntimeCapabilityKind,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeDecoderCandidate {
    pub plugin_id: String,
    pub type_id: String,
    pub score: u16,
}

struct RuntimeDecoderPluginCell {
    inner: WasmtimeDecoderPlugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeDecoderPlugin {
    id: u64,
}

static RUNTIME_DECODER_PLUGIN_SEQ: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static RUNTIME_DECODER_PLUGINS: RefCell<BTreeMap<u64, RuntimeDecoderPluginCell>> =
        RefCell::new(BTreeMap::new());
}

impl RuntimeDecoderPlugin {
    fn with_cell<T>(
        &self,
        mut apply: impl FnMut(&mut RuntimeDecoderPluginCell) -> std::result::Result<T, WasmPluginError>,
    ) -> std::result::Result<T, WasmPluginError> {
        RUNTIME_DECODER_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            let Some(cell) = map.get_mut(&self.id) else {
                return Err(WasmPluginError::operation(
                    "runtime.decoder",
                    format!("decoder handle `{}` not found in current thread", self.id),
                ));
            };
            apply(cell)
        })
    }

    pub fn open_stream(
        &mut self,
        stream: Box<dyn HostStreamHandle>,
        ext_hint: Option<&str>,
    ) -> std::result::Result<u64, WasmPluginError> {
        let mut stream = Some(stream);
        self.with_cell(|cell| {
            let stream = stream.take().ok_or_else(|| {
                WasmPluginError::operation("runtime.decoder.open_stream", "stream is missing")
            })?;
            cell.inner
                .open_stream(stream, ext_hint)
                .map(|handle: RuntimeDecoderSessionHandle| handle.0)
        })
    }

    pub fn open_file(
        &mut self,
        path: &Path,
        ext_hint: Option<&str>,
    ) -> std::result::Result<u64, WasmPluginError> {
        let stream = open_local_file_stream(path)?;
        self.open_stream(stream, ext_hint)
    }

    pub fn info(
        &mut self,
        session_handle: u64,
    ) -> std::result::Result<crate::runtime::model::RuntimeDecoderInfo, WasmPluginError> {
        self.with_cell(|cell| cell.inner.info(RuntimeDecoderSessionHandle(session_handle)))
    }

    pub fn metadata(
        &mut self,
        session_handle: u64,
    ) -> std::result::Result<RuntimeMediaMetadata, WasmPluginError> {
        self.with_cell(|cell| {
            cell.inner
                .metadata(RuntimeDecoderSessionHandle(session_handle))
        })
    }

    pub fn read_pcm_f32(
        &mut self,
        session_handle: u64,
        max_frames: u32,
    ) -> std::result::Result<RuntimePcmF32Chunk, WasmPluginError> {
        self.with_cell(|cell| {
            cell.inner
                .read_pcm_f32(RuntimeDecoderSessionHandle(session_handle), max_frames)
        })
    }

    pub fn seek_ms(
        &mut self,
        session_handle: u64,
        position_ms: u64,
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            cell.inner
                .seek_ms(RuntimeDecoderSessionHandle(session_handle), position_ms)
        })
    }

    pub fn close(&mut self, session_handle: u64) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            cell.inner
                .close(RuntimeDecoderSessionHandle(session_handle))
        })
    }
}

impl Drop for RuntimeDecoderPlugin {
    fn drop(&mut self) {
        RUNTIME_DECODER_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            map.remove(&self.id);
        });
    }
}

pub struct RuntimeSourcePlugin {
    inner: WasmtimeSourcePlugin,
}

impl RuntimeSourcePlugin {
    pub fn list_items_json(
        &mut self,
        request_json: &str,
    ) -> std::result::Result<String, WasmPluginError> {
        self.inner.list_items_json(request_json)
    }

    pub fn apply_config_update_json(
        &mut self,
        config_json: &str,
    ) -> std::result::Result<(), WasmPluginError> {
        self.inner.apply_config_update_json(config_json)
    }

    pub fn open_stream_json(
        &mut self,
        track_json: &str,
    ) -> std::result::Result<RuntimeOpenedSourceStream, WasmPluginError> {
        self.inner.open_stream_json(track_json)
    }

    pub fn read(
        &mut self,
        stream: RuntimeSourceStreamHandle,
        max_bytes: u32,
    ) -> std::result::Result<RuntimeEncodedChunk, WasmPluginError> {
        self.inner.read(stream, max_bytes)
    }

    pub fn close_stream(
        &mut self,
        stream: RuntimeSourceStreamHandle,
    ) -> std::result::Result<(), WasmPluginError> {
        self.inner.close_stream(stream)
    }
}

pub struct RuntimeLyricsPlugin {
    inner: WasmtimeLyricsPlugin,
}

impl RuntimeLyricsPlugin {
    pub fn search_json(&mut self, keyword: &str) -> std::result::Result<String, WasmPluginError> {
        let out = self.inner.search(keyword)?;
        serde_json::to_string(&out).map_err(WasmPluginError::from)
    }

    pub fn fetch_text(&mut self, lyric_id: &str) -> std::result::Result<String, WasmPluginError> {
        self.inner.fetch(lyric_id)
    }
}

struct RuntimeDspPluginCell {
    inner: WasmtimeDspPlugin,
    processor: Option<RuntimeDspProcessorHandle>,
    spec: Option<RuntimeAudioSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeDspPlugin {
    id: u64,
}

static RUNTIME_DSP_PLUGIN_SEQ: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static RUNTIME_DSP_PLUGINS: RefCell<BTreeMap<u64, RuntimeDspPluginCell>> =
        RefCell::new(BTreeMap::new());
}

impl RuntimeDspPlugin {
    fn with_cell<T>(
        &self,
        mut apply: impl FnMut(&mut RuntimeDspPluginCell) -> std::result::Result<T, WasmPluginError>,
    ) -> std::result::Result<T, WasmPluginError> {
        RUNTIME_DSP_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            let Some(cell) = map.get_mut(&self.id) else {
                return Err(WasmPluginError::operation(
                    "runtime.dsp",
                    format!("dsp handle `{}` not found in current thread", self.id),
                ));
            };
            apply(cell)
        })
    }

    pub fn open_processor(
        &mut self,
        sample_rate: u32,
        channels: u16,
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            if let Some(processor) = cell.processor.take() {
                let _ = cell.inner.close_processor(processor);
            }
            let spec = RuntimeAudioSpec {
                sample_rate: sample_rate.max(1),
                channels: channels.max(1),
            };
            let processor = cell.inner.create_processor(spec)?;
            cell.processor = Some(processor);
            cell.spec = Some(spec);
            Ok(())
        })
    }

    fn required_processor(
        cell: &RuntimeDspPluginCell,
    ) -> std::result::Result<RuntimeDspProcessorHandle, WasmPluginError> {
        cell.processor
            .ok_or_else(|| WasmPluginError::operation("runtime.dsp", "dsp processor is not open"))
    }

    pub fn apply_config_update_json(
        &mut self,
        config_json: &str,
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            let processor = Self::required_processor(cell)?;
            cell.inner.apply_config_update_json(processor, config_json)
        })
    }

    pub fn process_interleaved_f32_in_place(
        &mut self,
        channels: u16,
        samples: &mut [f32],
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            let processor = Self::required_processor(cell)?;
            let mut input = Vec::with_capacity(samples.len() * 4);
            for sample in samples.iter() {
                input.extend_from_slice(&sample.to_le_bytes());
            }
            let output = cell
                .inner
                .process_interleaved_f32(processor, channels, input)?;
            if output.len() != samples.len() * 4 {
                return Err(WasmPluginError::operation(
                    "runtime.dsp.process_interleaved_f32",
                    format!(
                        "output size mismatch: expected {} bytes, got {} bytes",
                        samples.len() * 4,
                        output.len()
                    ),
                ));
            }
            for (idx, bytes) in output.chunks_exact(4).enumerate() {
                samples[idx] = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            }
            Ok(())
        })
    }

    pub fn export_state_json(&mut self) -> std::result::Result<Option<String>, WasmPluginError> {
        self.with_cell(|cell| {
            let processor = Self::required_processor(cell)?;
            cell.inner.export_state_json(processor)
        })
    }

    pub fn import_state_json(
        &mut self,
        state_json: &str,
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            let processor = Self::required_processor(cell)?;
            cell.inner.import_state_json(processor, state_json)
        })
    }

    pub fn close_processor(&mut self) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            if let Some(processor) = cell.processor.take() {
                cell.inner.close_processor(processor)?;
            }
            cell.spec = None;
            Ok(())
        })
    }

    pub fn processor_spec(&self) -> Option<(u32, u16)> {
        self.with_cell(|cell| Ok(cell.spec.map(|spec| (spec.sample_rate, spec.channels))))
            .ok()
            .flatten()
    }
}

impl Drop for RuntimeDspPlugin {
    fn drop(&mut self) {
        RUNTIME_DSP_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            map.remove(&self.id);
        });
    }
}

struct RuntimeOutputSinkPluginCell {
    inner: WasmtimeOutputSinkPlugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuntimeOutputSinkPlugin {
    id: u64,
}

static RUNTIME_OUTPUT_SINK_PLUGIN_SEQ: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static RUNTIME_OUTPUT_SINK_PLUGINS: RefCell<BTreeMap<u64, RuntimeOutputSinkPluginCell>> =
        RefCell::new(BTreeMap::new());
}

impl RuntimeOutputSinkPlugin {
    fn with_cell<T>(
        &self,
        mut apply: impl FnMut(
            &mut RuntimeOutputSinkPluginCell,
        ) -> std::result::Result<T, WasmPluginError>,
    ) -> std::result::Result<T, WasmPluginError> {
        RUNTIME_OUTPUT_SINK_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            let Some(cell) = map.get_mut(&self.id) else {
                return Err(WasmPluginError::operation(
                    "runtime.output_sink",
                    format!(
                        "output sink handle `{}` not found in current thread",
                        self.id
                    ),
                ));
            };
            apply(cell)
        })
    }

    pub fn apply_config_update_json(
        &mut self,
        config_json: &str,
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| cell.inner.apply_config_update_json(config_json))
    }

    pub fn list_targets_json(&mut self) -> std::result::Result<String, WasmPluginError> {
        self.with_cell(|cell| cell.inner.list_targets_json())
    }

    pub fn negotiate_spec(
        &mut self,
        target_json: &str,
        sample_rate: u32,
        channels: u16,
    ) -> std::result::Result<RuntimeNegotiatedSpec, WasmPluginError> {
        self.with_cell(|cell| {
            cell.inner.negotiate_spec_json(
                target_json,
                RuntimeAudioSpec {
                    sample_rate,
                    channels,
                },
            )
        })
    }

    pub fn open(
        &mut self,
        target_json: &str,
        sample_rate: u32,
        channels: u16,
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| {
            cell.inner.open_json(
                target_json,
                RuntimeAudioSpec {
                    sample_rate,
                    channels,
                },
            )
        })
    }

    pub fn write_interleaved_f32(
        &mut self,
        channels: u16,
        samples: &[f32],
    ) -> std::result::Result<u32, WasmPluginError> {
        self.with_cell(|cell| {
            let mut bytes = Vec::with_capacity(samples.len() * 4);
            for sample in samples {
                bytes.extend_from_slice(&sample.to_le_bytes());
            }
            cell.inner.write_interleaved_f32(channels, bytes)
        })
    }

    pub fn query_status(
        &mut self,
    ) -> std::result::Result<RuntimeOutputSinkStatus, WasmPluginError> {
        self.with_cell(|cell| cell.inner.query_status())
    }

    pub fn flush(&mut self) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| cell.inner.flush())
    }

    pub fn export_state_json(&mut self) -> std::result::Result<Option<String>, WasmPluginError> {
        self.with_cell(|cell| cell.inner.export_state_json())
    }

    pub fn import_state_json(
        &mut self,
        state_json: &str,
    ) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| cell.inner.import_state_json(state_json))
    }

    pub fn close(&mut self) -> std::result::Result<(), WasmPluginError> {
        self.with_cell(|cell| cell.inner.close())
    }
}

impl Drop for RuntimeOutputSinkPlugin {
    fn drop(&mut self) {
        RUNTIME_OUTPUT_SINK_PLUGINS.with(|map| {
            let mut map = map.borrow_mut();
            map.remove(&self.id);
        });
    }
}

pub fn shared_runtime_service() -> runtime_service::SharedPluginRuntime {
    runtime_service::shared_runtime_service()
}
