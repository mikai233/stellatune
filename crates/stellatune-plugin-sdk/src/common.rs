use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeekWhence {
    Start,
    Current,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigUpdateMode {
    HotApply,
    Recreate,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigUpdatePlan {
    pub mode: ConfigUpdateMode,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisableReason {
    HostDisable,
    Unload,
    Shutdown,
    Reload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedAudioFormat {
    pub codec: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bitrate_kbps: Option<u32>,
    pub container: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AudioTags {
    pub title: Option<String>,
    pub album: Option<String>,
    pub artists: Vec<String>,
    pub album_artists: Vec<String>,
    pub genres: Vec<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataValue {
    Text(String),
    Boolean(bool),
    Uint32(u32),
    Uint64(u64),
    Int64(i64),
    Float64(f64),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: MetadataValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub tags: AudioTags,
    pub duration_ms: Option<u64>,
    pub format: EncodedAudioFormat,
    pub extras: Vec<MetadataEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedChunk {
    pub bytes: Vec<u8>,
    pub eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcmF32Chunk {
    pub interleaved_f32le: Vec<u8>,
    pub frames: u32,
    pub eof: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecoderInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub seekable: bool,
    pub encoder_delay_frames: u32,
    pub encoder_padding_frames: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedSpec {
    pub spec: AudioSpec,
    pub preferred_chunk_frames: u32,
    pub prefer_track_rate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSinkStatus {
    pub queued_samples: u32,
    pub running: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleFormat {
    F32Le,
    I16Le,
    I32Le,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotPathRole {
    DspTransform,
    OutputSink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferLayout {
    pub in_offset: u32,
    pub out_offset: Option<u32>,
    pub max_frames: u32,
    pub channels: u16,
    pub sample_format: SampleFormat,
    pub interleaved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreModuleSpec {
    pub role: HotPathRole,
    pub wasm_rel_path: String,
    pub abi_version: u32,
    pub memory_export: String,
    pub init_export: String,
    pub process_export: String,
    pub reset_export: Option<String>,
    pub drop_export: Option<String>,
    pub buffer: BufferLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricCandidate {
    pub id: String,
    pub title: String,
    pub artist: String,
}
