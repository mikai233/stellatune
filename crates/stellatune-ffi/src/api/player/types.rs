use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Buffering,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackSnapshot {
    pub state: PlayerState,
    pub track_id: Option<u64>,
    pub item_id: Option<u64>,
    pub local_library_track_id: Option<i64>,
    pub position_ms: i64,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LfeMode {
    #[default]
    Mute,
    MixToFront,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResampleQuality {
    Fast,
    Balanced,
    #[default]
    High,
    Ultra,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioBackend {
    Shared,
    WasapiExclusive,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioDevice {
    pub backend: AudioBackend,
    pub id: String,
    pub name: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DspChainItem {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
}

impl DspChainItem {
    pub fn with_config<T: Serialize>(
        plugin_id: String,
        type_id: String,
        config: &T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            plugin_id,
            type_id,
            config_json: serde_json::to_string(config)?,
        })
    }

    pub fn config<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.config_json)
    }
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DspTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceCatalogTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricsProviderTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSinkTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncoderTypeDescriptor {
    pub plugin_id: String,
    pub plugin_name: String,
    pub type_id: String,
    pub display_name: String,
    pub config_schema_json: String,
    pub default_config_json: String,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputSinkRoute {
    pub plugin_id: String,
    pub type_id: String,
    pub config_json: String,
    pub target_json: String,
}

impl OutputSinkRoute {
    pub fn with_config_target<C: Serialize, T: Serialize>(
        plugin_id: String,
        type_id: String,
        config: &C,
        target: &T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            plugin_id,
            type_id,
            config_json: serde_json::to_string(config)?,
            target_json: serde_json::to_string(target)?,
        })
    }

    pub fn config<C: DeserializeOwned>(&self) -> Result<C, serde_json::Error> {
        serde_json::from_str(&self.config_json)
    }

    pub fn target<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.target_json)
    }
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    StateChanged {
        state: PlayerState,
    },
    Position {
        ms: i64,
        track_id: u64,
        item_id: u64,
        session_id: u64,
    },
    TrackChanged {
        track_id: u64,
        item_id: u64,
    },
    PlaybackEnded {
        track_id: u64,
        item_id: u64,
    },
    VolumeChanged {
        volume: f32,
        seq: u64,
    },
    AudioStart,
    AudioEnd,
    Error {
        message: String,
    },
    Log {
        message: String,
    },
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackDecodeInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: Option<u64>,
    pub metadata_json: Option<String>,
    pub decoder_plugin_id: Option<String>,
    pub decoder_type_id: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscodeProgressEvent {
    pub phase: String,
    pub message: Option<String>,
    pub source_path: Option<String>,
    pub output_path: Option<String>,
    pub processed_frames: u64,
    pub total_frames: Option<u64>,
    pub written_bytes: u64,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub elapsed_ms: Option<u64>,
}

impl TrackDecodeInfo {
    pub fn set_metadata<T: Serialize>(
        &mut self,
        metadata: Option<&T>,
    ) -> Result<(), serde_json::Error> {
        self.metadata_json = match metadata {
            Some(v) => Some(serde_json::to_string(v)?),
            None => None,
        };
        Ok(())
    }

    pub fn metadata<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        let Some(raw) = self.metadata_json.as_deref() else {
            return Ok(None);
        };
        serde_json::from_str(raw).map(Some)
    }
}
