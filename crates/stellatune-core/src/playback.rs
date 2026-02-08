use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::protocol::PluginRuntimeKind;

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Buffering,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LfeMode {
    #[default]
    Mute,
    MixToFront,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackRef {
    /// Logical source id (e.g. `local`, `netease`, `onedrive`).
    pub source_id: String,
    /// Stable identifier inside the source.
    pub track_id: String,
    /// Opaque locator used by source/decoder implementations.
    pub locator: String,
}

impl TrackRef {
    pub fn new(source_id: String, track_id: String, locator: String) -> Self {
        Self {
            source_id,
            track_id,
            locator,
        }
    }

    pub fn for_local_path(path: String) -> Self {
        Self {
            source_id: "local".to_string(),
            track_id: path.clone(),
            locator: path,
        }
    }

    pub fn stable_key(&self) -> String {
        format!("{}:{}", self.source_id, self.track_id)
    }
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackPlayability {
    pub track: TrackRef,
    pub playable: bool,
    pub reason: Option<String>,
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    LoadTrack {
        path: String,
    },
    LoadTrackRef {
        track: TrackRef,
    },
    Play,
    Pause,
    SeekMs {
        position_ms: u64,
    },
    SetVolume {
        volume: f32,
    },
    SetLfeMode {
        mode: LfeMode,
    },
    Stop,
    Shutdown,
    SetOutputDevice {
        backend: AudioBackend,
        device_id: Option<String>,
    },
    SetOutputOptions {
        match_track_sample_rate: bool,
        gapless_playback: bool,
        seek_track_fade: bool,
    },
    SetOutputSinkRoute {
        route: OutputSinkRoute,
    },
    ClearOutputSinkRoute,
    PreloadTrack {
        path: String,
        position_ms: u64,
    },
    PreloadTrackRef {
        track: TrackRef,
        position_ms: u64,
    },
    RefreshDevices,
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
    pub config_schema_json: String,
    pub default_config_json: String,
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
    StateChanged { state: PlayerState },
    Position { ms: i64 },
    TrackChanged { path: String },
    PlaybackEnded { path: String },
    VolumeChanged { volume: f32 },
    Error { message: String },
    Log { message: String },
    OutputDevicesChanged { devices: Vec<AudioDevice> },
}

#[flutter_rust_bridge::frb(non_opaque)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRuntimeEvent {
    pub plugin_id: String,
    pub kind: PluginRuntimeKind,
    pub payload_json: String,
}

impl PluginRuntimeEvent {
    pub fn from_payload<T: Serialize>(
        plugin_id: impl Into<String>,
        kind: PluginRuntimeKind,
        payload: &T,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            plugin_id: plugin_id.into(),
            kind,
            payload_json: serde_json::to_string(payload)?,
        })
    }

    pub fn payload<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.payload_json)
    }
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
