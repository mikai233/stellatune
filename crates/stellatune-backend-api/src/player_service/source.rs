use std::collections::BTreeMap;
use std::path::PathBuf;

use stellatune_audio_core::source::MediaHints;

use super::error::PlayerServiceError;
use super::identity::{
    MAX_PROVIDER_ID_BYTES, ProviderTrackKey, SourceBinding, SourceInstanceId, TrackId,
};

const MAX_MEDIA_HINT_BYTES: usize = 255;
pub enum TrackOrigin {
    LocalLibrary { library_track_id: i64 },
    Provider(ProviderTrackKey),
}

pub struct SourceCatalogEntry {
    pub id: SourceInstanceId,
    pub binding: SourceBinding,
    pub resolver: Option<SourceResolverSpec>,
    pub tombstoned: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SourceResolverSpec {
    pub plugin_id: String,
    pub capability_id: String,
    pub config_json: String,
}

impl std::fmt::Debug for SourceResolverSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceResolverSpec")
            .field("plugin_id", &self.plugin_id)
            .field("capability_id", &self.capability_id)
            .field("config_json", &"<redacted resolver config>")
            .finish()
    }
}

impl SourceResolverSpec {
    pub fn new(
        plugin_id: impl Into<String>,
        capability_id: impl Into<String>,
        config_json: impl Into<String>,
    ) -> Result<Self, PlayerServiceError> {
        let plugin_id = plugin_id.into();
        let capability_id = capability_id.into();
        let config_json = config_json.into();
        if plugin_id.trim().is_empty()
            || capability_id.trim().is_empty()
            || plugin_id.trim() != plugin_id
            || capability_id.trim() != capability_id
            || plugin_id.len() > MAX_PROVIDER_ID_BYTES
            || capability_id.len() > MAX_PROVIDER_ID_BYTES
        {
            return Err(PlayerServiceError::InvalidSourceSpec(
                "invalid persisted source resolver identity".to_owned(),
            ));
        }
        let config: serde_json::Value = serde_json::from_str(&config_json)
            .map_err(|error| PlayerServiceError::InvalidSourceSpec(error.to_string()))?;
        if !config.is_object() {
            return Err(PlayerServiceError::InvalidSourceSpec(
                "source resolver config must be a JSON object".to_owned(),
            ));
        }
        Ok(Self {
            plugin_id,
            capability_id,
            config_json,
        })
    }
}

pub struct TrackCatalogEntry {
    pub id: TrackId,
    pub source: SourceInstanceId,
    pub origin: TrackOrigin,
    pub tombstoned: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaHintsInput {
    pub extension: Option<String>,
    pub mime_type: Option<String>,
    pub content_length: Option<u64>,
    pub container_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceResolutionInput {
    File {
        path: String,
        media: MediaHintsInput,
    },
    Http {
        url: String,
        headers: BTreeMap<String, String>,
        media: MediaHintsInput,
        seekable: bool,
        live: bool,
    },
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResolvedSourceSpec {
    File {
        path: PathBuf,
        media: MediaHints,
    },
    Http {
        url: String,
        headers: BTreeMap<String, String>,
        media: MediaHints,
        capabilities: stellatune_audio_core::source::SourceCapabilities,
    },
}

impl std::fmt::Debug for ResolvedSourceSpec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File { path, media } => formatter
                .debug_struct("ResolvedSourceSpec::File")
                .field("path", path)
                .field("media", media)
                .finish(),
            Self::Http {
                url,
                headers,
                media,
                capabilities,
            } => formatter
                .debug_struct("ResolvedSourceSpec::Http")
                .field(
                    "url",
                    &format_args!("<redacted HTTP URL, {} bytes>", url.len()),
                )
                .field(
                    "headers",
                    &format_args!("<{} redacted headers>", headers.len()),
                )
                .field("media", media)
                .field("capabilities", capabilities)
                .finish(),
        }
    }
}

impl TryFrom<SourceResolutionInput> for ResolvedSourceSpec {
    type Error = PlayerServiceError;

    fn try_from(input: SourceResolutionInput) -> Result<Self, Self::Error> {
        match input {
            SourceResolutionInput::File { path, media } => {
                let path = PathBuf::from(path.trim());
                if path.as_os_str().is_empty() {
                    return Err(PlayerServiceError::InvalidSourceSpec(
                        "file path cannot be empty".to_owned(),
                    ));
                }
                Ok(Self::File {
                    path,
                    media: validate_media_hints(media)?,
                })
            },
            SourceResolutionInput::Http {
                url,
                headers,
                media,
                seekable,
                live,
            } => {
                let parsed = url::Url::parse(url.trim())
                    .map_err(|error| PlayerServiceError::InvalidSourceSpec(error.to_string()))?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err(PlayerServiceError::InvalidSourceSpec(
                        "URL must use http or https".to_owned(),
                    ));
                }
                for (name, value) in &headers {
                    reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                        PlayerServiceError::InvalidSourceSpec(error.to_string())
                    })?;
                    reqwest::header::HeaderValue::from_str(value).map_err(|error| {
                        PlayerServiceError::InvalidSourceSpec(error.to_string())
                    })?;
                }
                Ok(Self::Http {
                    url: parsed.to_string(),
                    headers,
                    media: validate_media_hints(media)?,
                    capabilities: stellatune_audio_core::source::SourceCapabilities {
                        byte_seekable: seekable && !live,
                        reopenable: true,
                        live,
                    },
                })
            },
        }
    }
}

fn validate_media_hints(input: MediaHintsInput) -> Result<MediaHints, PlayerServiceError> {
    fn clean(value: Option<String>) -> Result<Option<String>, PlayerServiceError> {
        value
            .map(|value| {
                let value = value.trim();
                if value.is_empty() || value.len() > MAX_MEDIA_HINT_BYTES {
                    return Err(PlayerServiceError::InvalidSourceSpec(
                        "invalid media hint".to_owned(),
                    ));
                }
                Ok(value.to_owned())
            })
            .transpose()
    }
    Ok(MediaHints {
        extension: clean(input.extension)?
            .map(|value| value.trim_start_matches('.').to_ascii_lowercase()),
        mime_type: clean(input.mime_type)?.map(|value| value.to_ascii_lowercase()),
        content_length: input.content_length,
        container_hint: clean(input.container_hint)?,
    })
}
