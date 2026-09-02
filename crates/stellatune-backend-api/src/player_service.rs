use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use stellatune_audio::playback::{PlaybackController, PlaybackRuntimeSnapshot, SwitchOptions};
use stellatune_audio_builtin_adapters::factories::{FileSourceFactory, HttpSourceFactory};
use stellatune_audio_core::{
    DecoderFactory, MediaHints, PlaybackItem, PlaybackItemId, SourceFactory,
};
use thiserror::Error;

const PLAYER_SCHEMA_VERSION: i64 = 1;
const MAX_PERSISTED_QUEUE_ITEMS: i64 = 1_024;
const PLAYER_SCHEMA_FINGERPRINT: &str = "stellatune-player-v1-typed-catalog-state-20260901";
const MAX_PROVIDER_TEXT_KEY_BYTES: usize = 512;
const MAX_PROVIDER_ID_BYTES: usize = 255;
const MAX_MEDIA_HINT_BYTES: usize = 255;

macro_rules! persistent_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);

        impl $name {
            pub fn new(value: u64) -> Result<Self, PlayerServiceError> {
                if value == 0 || value > i64::MAX as u64 {
                    return Err(PlayerServiceError::InvalidIdentity {
                        identity: stringify!($name),
                        value,
                    });
                }
                Ok(Self(NonZeroU64::new(value).expect("non-zero validated")))
            }

            pub const fn get(self) -> u64 {
                self.0.get()
            }

            fn as_i64(self) -> i64 {
                self.get() as i64
            }
        }
    };
}

persistent_id!(SourceInstanceId);
persistent_id!(TrackId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    LocalLibrary,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Result<Self, PlayerServiceError> {
        let value = value.into();
        let normalized = value.trim();
        if normalized.is_empty() || normalized.len() > MAX_PROVIDER_ID_BYTES {
            return Err(PlayerServiceError::InvalidProviderId);
        }
        Ok(Self(normalized.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceBinding {
    LocalLibrary,
    Plugin { provider_id: ProviderId },
}

impl SourceBinding {
    pub fn kind(&self) -> SourceKind {
        match self {
            Self::LocalLibrary => SourceKind::LocalLibrary,
            Self::Plugin { .. } => SourceKind::Plugin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderTrackKey {
    Numeric(u64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderTrackKeyInput {
    Numeric(u64),
    Text(String),
}

impl TryFrom<ProviderTrackKeyInput> for ProviderTrackKey {
    type Error = PlayerServiceError;

    fn try_from(value: ProviderTrackKeyInput) -> Result<Self, Self::Error> {
        match value {
            ProviderTrackKeyInput::Numeric(value) if value > 0 && value <= i64::MAX as u64 => {
                Ok(Self::Numeric(value))
            },
            ProviderTrackKeyInput::Numeric(value) => Err(PlayerServiceError::InvalidIdentity {
                identity: "ProviderTrackKey::Numeric",
                value,
            }),
            ProviderTrackKeyInput::Text(value) => {
                if value.is_empty()
                    || value.trim() != value
                    || value.len() > MAX_PROVIDER_TEXT_KEY_BYTES
                {
                    return Err(PlayerServiceError::InvalidProviderTrackKey);
                }
                Ok(Self::Text(value))
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTrackIdentityInput {
    pub source_instance_id: u64,
    pub provider_key: ProviderTrackKeyInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderTrackIdentity {
    pub source_instance_id: SourceInstanceId,
    pub provider_key: ProviderTrackKey,
}

impl TryFrom<ProviderTrackIdentityInput> for ProviderTrackIdentity {
    type Error = PlayerServiceError;

    fn try_from(value: ProviderTrackIdentityInput) -> Result<Self, Self::Error> {
        Ok(Self {
            source_instance_id: SourceInstanceId::new(value.source_instance_id)?,
            provider_key: value.provider_key.try_into()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackOrigin {
    LocalLibrary { library_track_id: i64 },
    Provider(ProviderTrackKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
        capabilities: stellatune_audio_core::SourceCapabilities,
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
                    capabilities: stellatune_audio_core::SourceCapabilities {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackQueueRecord {
    pub item_id: PlaybackItemId,
    pub track_id: TrackId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackStateRecord {
    pub schema_version: u32,
    pub queue: Vec<PlaybackQueueRecord>,
    pub current_item_id: Option<PlaybackItemId>,
    pub position_ms: u64,
    pub repeat_mode: RepeatMode,
    pub shuffle_enabled: bool,
    pub was_playing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    fn parse(value: &str) -> Result<Self, PlayerServiceError> {
        match value {
            "off" => Ok(Self::Off),
            "all" => Ok(Self::All),
            "one" => Ok(Self::One),
            _ => Err(PlayerServiceError::IncompatiblePlaybackSchema),
        }
    }
}

#[derive(Clone)]
pub struct PlayerCatalog {
    pool: SqlitePool,
}

impl PlayerCatalog {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, PlayerServiceError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let catalog = Self { pool };
        catalog.validate_or_bootstrap().await?;
        Ok(catalog)
    }

    async fn validate_or_bootstrap(&self) -> Result<(), PlayerServiceError> {
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name IN \
             ('player_schema_meta','source_catalog','track_catalog','playback_queue','playback_state')",
        )
        .fetch_all(&self.pool)
        .await?;
        let names = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();
        if names.is_empty() {
            return self.bootstrap().await;
        }
        if !names.iter().any(|name| name == "player_schema_meta") || names.len() != 5 {
            return Err(PlayerServiceError::IncompatiblePlaybackSchema);
        }
        let meta = sqlx::query(
            "SELECT schema_version,schema_fingerprint FROM player_schema_meta WHERE singleton=1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| PlayerServiceError::IncompatiblePlaybackSchema)?;
        let Some(meta) = meta else {
            return Err(PlayerServiceError::IncompatiblePlaybackSchema);
        };
        if meta.get::<i64, _>("schema_version") != PLAYER_SCHEMA_VERSION
            || meta.get::<String, _>("schema_fingerprint") != PLAYER_SCHEMA_FINGERPRINT
        {
            return Err(PlayerServiceError::IncompatiblePlaybackSchema);
        }
        let foreign_key_failures = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&self.pool)
            .await?;
        if !foreign_key_failures.is_empty() {
            return Err(PlayerServiceError::IncompatiblePlaybackSchema);
        }
        Ok(())
    }

    async fn bootstrap(&self) -> Result<(), PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        for statement in [
            "CREATE TABLE player_schema_meta(\
                singleton INTEGER PRIMARY KEY CHECK(singleton=1),\
                schema_version INTEGER NOT NULL,\
                schema_fingerprint TEXT NOT NULL)",
            "CREATE TABLE source_catalog(\
                id INTEGER PRIMARY KEY AUTOINCREMENT CHECK(id>0),\
                binding_kind TEXT NOT NULL CHECK(binding_kind IN ('local','plugin')),\
                provider_id TEXT, resolver_plugin_id TEXT, resolver_capability_id TEXT,\
                resolver_config_json TEXT, tombstoned INTEGER NOT NULL DEFAULT 0,\
                CHECK((binding_kind='local' AND provider_id IS NULL AND resolver_plugin_id IS NULL \
                       AND resolver_capability_id IS NULL AND resolver_config_json IS NULL) OR \
                      (binding_kind='plugin' AND provider_id IS NOT NULL AND resolver_plugin_id IS NOT NULL \
                       AND resolver_capability_id IS NOT NULL AND resolver_config_json IS NOT NULL)))",
            "CREATE UNIQUE INDEX source_catalog_local_unique ON source_catalog(binding_kind) \
                WHERE binding_kind='local'",
            "CREATE UNIQUE INDEX source_catalog_provider_unique ON source_catalog(provider_id) \
                WHERE provider_id IS NOT NULL",
            "CREATE TABLE track_catalog(\
                id INTEGER PRIMARY KEY AUTOINCREMENT CHECK(id>0),\
                source_id INTEGER NOT NULL REFERENCES source_catalog(id),\
                origin_kind TEXT NOT NULL CHECK(origin_kind IN ('local','provider_numeric','provider_text')),\
                local_library_id INTEGER, provider_numeric INTEGER, provider_text TEXT,\
                tombstoned INTEGER NOT NULL DEFAULT 0,\
                CHECK((origin_kind='local' AND local_library_id IS NOT NULL AND provider_numeric IS NULL AND provider_text IS NULL) OR\
                      (origin_kind='provider_numeric' AND local_library_id IS NULL AND provider_numeric IS NOT NULL AND provider_text IS NULL) OR\
                      (origin_kind='provider_text' AND local_library_id IS NULL AND provider_numeric IS NULL AND provider_text IS NOT NULL)))",
            "CREATE UNIQUE INDEX track_catalog_local_unique ON track_catalog(source_id,local_library_id) \
                WHERE origin_kind='local'",
            "CREATE UNIQUE INDEX track_catalog_numeric_unique ON track_catalog(source_id,provider_numeric) \
                WHERE origin_kind='provider_numeric'",
            "CREATE UNIQUE INDEX track_catalog_text_unique ON track_catalog(source_id,provider_text) \
                WHERE origin_kind='provider_text'",
            "CREATE TABLE playback_queue(\
                item_id INTEGER PRIMARY KEY AUTOINCREMENT CHECK(item_id>0),\
                track_id INTEGER NOT NULL REFERENCES track_catalog(id),\
                queue_position INTEGER NOT NULL UNIQUE CHECK(queue_position>=0))",
            "CREATE TABLE playback_state(\
                singleton INTEGER PRIMARY KEY CHECK(singleton=1),\
                schema_version INTEGER NOT NULL, current_item_id INTEGER REFERENCES playback_queue(item_id),\
                position_ms INTEGER NOT NULL DEFAULT 0 CHECK(position_ms>=0),\
                repeat_mode TEXT NOT NULL DEFAULT 'off' CHECK(repeat_mode IN ('off','all','one')),\
                shuffle_enabled INTEGER NOT NULL DEFAULT 0 CHECK(shuffle_enabled IN (0,1)),\
                was_playing INTEGER NOT NULL DEFAULT 0)",
        ] {
            sqlx::query(statement).execute(&mut *tx).await?;
        }
        sqlx::query(
            "INSERT INTO player_schema_meta(singleton,schema_version,schema_fingerprint) VALUES(1,?,?)",
        )
            .bind(PLAYER_SCHEMA_VERSION)
            .bind(PLAYER_SCHEMA_FINGERPRINT)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO playback_state(singleton,schema_version,position_ms,repeat_mode,shuffle_enabled,was_playing) \
             VALUES(1,?,0,'off',0,0)",
        )
        .bind(PLAYER_SCHEMA_VERSION)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn ensure_local_source(&self) -> Result<SourceInstanceId, PlayerServiceError> {
        sqlx::query("INSERT OR IGNORE INTO source_catalog(binding_kind) VALUES('local')")
            .execute(&self.pool)
            .await?;
        let id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM source_catalog WHERE binding_kind='local'",
        )
        .fetch_one(&self.pool)
        .await?;
        SourceInstanceId::new(id as u64)
    }

    pub async fn ensure_plugin_source(
        &self,
        provider_id: ProviderId,
        resolver: SourceResolverSpec,
    ) -> Result<SourceInstanceId, PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT OR IGNORE INTO source_catalog(\
                binding_kind,provider_id,resolver_plugin_id,resolver_capability_id,resolver_config_json\
             ) VALUES('plugin',?,?,?,?)",
        )
        .bind(provider_id.as_str())
        .bind(&resolver.plugin_id)
        .bind(&resolver.capability_id)
        .bind(&resolver.config_json)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE source_catalog SET resolver_plugin_id=?,resolver_capability_id=?,\
                    resolver_config_json=? WHERE binding_kind='plugin' AND provider_id=?",
        )
        .bind(&resolver.plugin_id)
        .bind(&resolver.capability_id)
        .bind(&resolver.config_json)
        .bind(provider_id.as_str())
        .execute(&mut *tx)
        .await?;
        let id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM source_catalog WHERE binding_kind='plugin' AND provider_id=?",
        )
        .bind(provider_id.as_str())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        SourceInstanceId::new(id as u64)
    }

    pub async fn ensure_local_track(
        &self,
        source: SourceInstanceId,
        library_track_id: i64,
    ) -> Result<TrackId, PlayerServiceError> {
        let entry = self.source(source).await?;
        if entry.binding != SourceBinding::LocalLibrary || library_track_id <= 0 {
            return Err(PlayerServiceError::CatalogBindingMismatch);
        }
        sqlx::query(
            "INSERT OR IGNORE INTO track_catalog(source_id,origin_kind,local_library_id) \
             VALUES(?,'local',?)",
        )
        .bind(source.as_i64())
        .bind(library_track_id)
        .execute(&self.pool)
        .await?;
        let id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM track_catalog WHERE source_id=? AND origin_kind='local' AND local_library_id=?",
        )
        .bind(source.as_i64())
        .bind(library_track_id)
        .fetch_one(&self.pool)
        .await?;
        TrackId::new(id as u64)
    }

    pub async fn ensure_track(
        &self,
        identity: ProviderTrackIdentity,
    ) -> Result<TrackId, PlayerServiceError> {
        let source = self.source(identity.source_instance_id).await?;
        if !matches!(source.binding, SourceBinding::Plugin { .. }) {
            return Err(PlayerServiceError::CatalogBindingMismatch);
        }
        let (kind, numeric, text) = match &identity.provider_key {
            ProviderTrackKey::Numeric(value) => ("provider_numeric", Some(*value as i64), None),
            ProviderTrackKey::Text(value) => ("provider_text", None, Some(value.as_str())),
        };
        sqlx::query(
            "INSERT OR IGNORE INTO track_catalog(source_id,origin_kind,provider_numeric,provider_text) \
             VALUES(?,?,?,?)",
        )
        .bind(identity.source_instance_id.as_i64())
        .bind(kind)
        .bind(numeric)
        .bind(text)
        .execute(&self.pool)
        .await?;
        let id = match identity.provider_key {
            ProviderTrackKey::Numeric(value) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM track_catalog WHERE source_id=? AND origin_kind='provider_numeric' AND provider_numeric=?",
                )
                .bind(identity.source_instance_id.as_i64())
                .bind(value as i64)
                .fetch_one(&self.pool)
                .await?
            },
            ProviderTrackKey::Text(value) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM track_catalog WHERE source_id=? AND origin_kind='provider_text' AND provider_text=?",
                )
                .bind(identity.source_instance_id.as_i64())
                .bind(value)
                .fetch_one(&self.pool)
                .await?
            },
        };
        TrackId::new(id as u64)
    }

    pub async fn source(
        &self,
        id: SourceInstanceId,
    ) -> Result<SourceCatalogEntry, PlayerServiceError> {
        let row = sqlx::query(
            "SELECT binding_kind,provider_id,resolver_plugin_id,resolver_capability_id,\
                    resolver_config_json,tombstoned FROM source_catalog WHERE id=?",
        )
        .bind(id.as_i64())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PlayerServiceError::SourceNotFound(id))?;
        let kind: String = row.get("binding_kind");
        let binding = match kind.as_str() {
            "local" => SourceBinding::LocalLibrary,
            "plugin" => SourceBinding::Plugin {
                provider_id: ProviderId::new(row.get::<String, _>("provider_id"))?,
            },
            _ => return Err(PlayerServiceError::IncompatiblePlaybackSchema),
        };
        let resolver = match &binding {
            SourceBinding::LocalLibrary => None,
            SourceBinding::Plugin { .. } => Some(SourceResolverSpec::new(
                row.get::<String, _>("resolver_plugin_id"),
                row.get::<String, _>("resolver_capability_id"),
                row.get::<String, _>("resolver_config_json"),
            )?),
        };
        Ok(SourceCatalogEntry {
            id,
            binding,
            resolver,
            tombstoned: row.get::<i64, _>("tombstoned") != 0,
        })
    }

    pub async fn track(&self, id: TrackId) -> Result<TrackCatalogEntry, PlayerServiceError> {
        let row = sqlx::query(
            "SELECT source_id,origin_kind,local_library_id,provider_numeric,provider_text,tombstoned \
             FROM track_catalog WHERE id=?",
        )
        .bind(id.as_i64())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PlayerServiceError::TrackNotFound(id))?;
        let source = SourceInstanceId::new(row.get::<i64, _>("source_id") as u64)?;
        let origin = match row.get::<String, _>("origin_kind").as_str() {
            "local" => TrackOrigin::LocalLibrary {
                library_track_id: row.get("local_library_id"),
            },
            "provider_numeric" => TrackOrigin::Provider(ProviderTrackKey::Numeric(
                row.get::<i64, _>("provider_numeric") as u64,
            )),
            "provider_text" => {
                TrackOrigin::Provider(ProviderTrackKey::Text(row.get("provider_text")))
            },
            _ => return Err(PlayerServiceError::IncompatiblePlaybackSchema),
        };
        let source_entry = self.source(source).await?;
        if !matches!(
            (&source_entry.binding, &origin),
            (
                SourceBinding::LocalLibrary,
                TrackOrigin::LocalLibrary { .. }
            ) | (SourceBinding::Plugin { .. }, TrackOrigin::Provider(_))
        ) {
            return Err(PlayerServiceError::CatalogBindingMismatch);
        }
        Ok(TrackCatalogEntry {
            id,
            source,
            origin,
            tombstoned: row.get::<i64, _>("tombstoned") != 0,
        })
    }

    pub async fn tombstone_track(&self, id: TrackId) -> Result<(), PlayerServiceError> {
        let result = sqlx::query("UPDATE track_catalog SET tombstoned=1 WHERE id=?")
            .bind(id.as_i64())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() != 1 {
            return Err(PlayerServiceError::TrackNotFound(id));
        }
        Ok(())
    }

    pub async fn tombstone_source(&self, id: SourceInstanceId) -> Result<(), PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query("UPDATE source_catalog SET tombstoned=1 WHERE id=?")
            .bind(id.as_i64())
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() != 1 {
            return Err(PlayerServiceError::SourceNotFound(id));
        }
        sqlx::query("UPDATE track_catalog SET tombstoned=1 WHERE source_id=?")
            .bind(id.as_i64())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn enqueue(&self, track_id: TrackId) -> Result<PlaybackItemId, PlayerServiceError> {
        if self.track(track_id).await?.tombstoned {
            return Err(PlayerServiceError::TrackUnavailable(track_id));
        }
        let mut tx = self.pool.begin().await?;
        let position = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(queue_position)+1,0) FROM playback_queue",
        )
        .fetch_one(&mut *tx)
        .await?;
        let result = sqlx::query("INSERT INTO playback_queue(track_id,queue_position) VALUES(?,?)")
            .bind(track_id.as_i64())
            .bind(position)
            .execute(&mut *tx)
            .await?;
        let queue_len = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM playback_queue")
            .fetch_one(&mut *tx)
            .await?;
        let excess = queue_len.saturating_sub(MAX_PERSISTED_QUEUE_ITEMS);
        if excess > 0 {
            sqlx::query(
                "DELETE FROM playback_queue WHERE item_id IN (\
                    SELECT item_id FROM playback_queue \
                    WHERE item_id != COALESCE((SELECT current_item_id FROM playback_state WHERE singleton=1),-1) \
                    ORDER BY queue_position LIMIT ?)",
            )
            .bind(excess)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        PlaybackItemId::new(result.last_insert_rowid() as u64).ok_or(
            PlayerServiceError::InvalidIdentity {
                identity: "PlaybackItemId",
                value: result.last_insert_rowid() as u64,
            },
        )
    }

    pub async fn track_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<TrackId, PlayerServiceError> {
        let track_id =
            sqlx::query_scalar::<_, i64>("SELECT track_id FROM playback_queue WHERE item_id=?")
                .bind(item_id.get() as i64)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(PlayerServiceError::PlaybackItemNotFound(item_id))?;
        TrackId::new(track_id as u64)
    }

    pub async fn save_runtime_state(
        &self,
        snapshot: PlaybackRuntimeSnapshot,
        was_playing: bool,
    ) -> Result<(), PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        if let Some(item_id) = snapshot.current_item_id {
            let exists =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM playback_queue WHERE item_id=?")
                    .bind(item_id.get() as i64)
                    .fetch_one(&mut *tx)
                    .await?;
            if exists != 1 {
                return Err(PlayerServiceError::PlaybackStateInvariant);
            }
        }
        sqlx::query(
            "UPDATE playback_state SET current_item_id=?,position_ms=?,was_playing=? WHERE singleton=1",
        )
        .bind(snapshot.current_item_id.map(|id| id.get() as i64))
        .bind(snapshot.consumed_position.as_millis().min(i64::MAX as u64) as i64)
        .bind(i64::from(was_playing))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn load_state(&self) -> Result<PlaybackStateRecord, PlayerServiceError> {
        let state = sqlx::query(
            "SELECT schema_version,current_item_id,position_ms,repeat_mode,shuffle_enabled,was_playing \
             FROM playback_state WHERE singleton=1",
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PlayerServiceError::IncompatiblePlaybackSchema)?;
        if state.get::<i64, _>("schema_version") != PLAYER_SCHEMA_VERSION {
            return Err(PlayerServiceError::IncompatiblePlaybackSchema);
        }
        let rows =
            sqlx::query("SELECT item_id,track_id FROM playback_queue ORDER BY queue_position")
                .fetch_all(&self.pool)
                .await?;
        let queue = rows
            .into_iter()
            .map(|row| {
                let raw_item = row.get::<i64, _>("item_id") as u64;
                let raw_track = row.get::<i64, _>("track_id") as u64;
                Ok(PlaybackQueueRecord {
                    item_id: PlaybackItemId::new(raw_item).ok_or(
                        PlayerServiceError::InvalidIdentity {
                            identity: "PlaybackItemId",
                            value: raw_item,
                        },
                    )?,
                    track_id: TrackId::new(raw_track)?,
                })
            })
            .collect::<Result<Vec<_>, PlayerServiceError>>()?;
        let current_item_id = state
            .get::<Option<i64>, _>("current_item_id")
            .map(|value| {
                PlaybackItemId::new(value as u64).ok_or(PlayerServiceError::InvalidIdentity {
                    identity: "PlaybackItemId",
                    value: value as u64,
                })
            })
            .transpose()?;
        if current_item_id
            .is_some_and(|current| !queue.iter().any(|entry| entry.item_id == current))
        {
            return Err(PlayerServiceError::PlaybackStateInvariant);
        }
        Ok(PlaybackStateRecord {
            schema_version: PLAYER_SCHEMA_VERSION as u32,
            queue,
            current_item_id,
            position_ms: state.get::<i64, _>("position_ms") as u64,
            repeat_mode: RepeatMode::parse(&state.get::<String, _>("repeat_mode"))?,
            shuffle_enabled: state.get::<i64, _>("shuffle_enabled") != 0,
            was_playing: state.get::<i64, _>("was_playing") != 0,
        })
    }
}

#[derive(Clone)]
pub struct PlaybackStateStore {
    catalog: PlayerCatalog,
}

impl PlaybackStateStore {
    pub fn from_catalog(catalog: PlayerCatalog) -> Self {
        Self { catalog }
    }

    pub async fn enqueue(&self, track_id: TrackId) -> Result<PlaybackItemId, PlayerServiceError> {
        self.catalog.enqueue(track_id).await
    }

    pub async fn track_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<TrackId, PlayerServiceError> {
        self.catalog.track_for_item(item_id).await
    }

    pub async fn save_runtime_state(
        &self,
        snapshot: PlaybackRuntimeSnapshot,
        was_playing: bool,
    ) -> Result<(), PlayerServiceError> {
        self.catalog.save_runtime_state(snapshot, was_playing).await
    }

    pub async fn load(&self) -> Result<PlaybackStateRecord, PlayerServiceError> {
        self.catalog.load_state().await
    }
}

#[async_trait]
pub trait LocalTrackResolver: Send + Sync {
    async fn resolve_path(&self, library_track_id: i64) -> Result<PathBuf, PlayerServiceError>;
}

#[async_trait]
impl LocalTrackResolver for stellatune_library::LibraryHandle {
    async fn resolve_path(&self, library_track_id: i64) -> Result<PathBuf, PlayerServiceError> {
        self.get_track(library_track_id)
            .await
            .map_err(|error| PlayerServiceError::LocalLibrary(error.to_string()))?
            .map(|track| PathBuf::from(track.path))
            .ok_or(PlayerServiceError::LocalTrackNotFound(library_track_id))
    }
}

#[async_trait]
pub trait SourceResolver: Send + Sync {
    async fn resolve(
        &self,
        source: &SourceCatalogEntry,
        key: &ProviderTrackKey,
    ) -> Result<ResolvedSourceSpec, PlayerServiceError>;

    fn required_decoder(&self) -> Option<Arc<dyn DecoderFactory>> {
        None
    }
}

pub trait SourceResolverFactory: Send + Sync {
    fn create(
        &self,
        spec: &SourceResolverSpec,
    ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError>;
}

pub struct SourceMaterializer;

impl SourceMaterializer {
    pub fn materialize(
        &self,
        spec: ResolvedSourceSpec,
    ) -> Result<Arc<dyn SourceFactory>, PlayerServiceError> {
        match spec {
            ResolvedSourceSpec::File { path, media } => Ok(Arc::new(
                FileSourceFactory::new(path, media)
                    .map_err(|error| PlayerServiceError::Materialize(error.to_string()))?,
            )),
            ResolvedSourceSpec::Http {
                url,
                headers,
                media,
                capabilities,
            } => Ok(Arc::new(
                HttpSourceFactory::new(url, headers, media, capabilities)
                    .map_err(|error| PlayerServiceError::Materialize(error.to_string()))?,
            )),
        }
    }
}

pub struct PlayerService {
    catalog: PlayerCatalog,
    state_store: PlaybackStateStore,
    controller: PlaybackController,
    local_resolver: Arc<dyn LocalTrackResolver>,
    resolver_factory: Arc<dyn SourceResolverFactory>,
    source_resolvers: tokio::sync::RwLock<HashMap<SourceInstanceId, Arc<dyn SourceResolver>>>,
    materializer: SourceMaterializer,
    state_writer_started: AtomicBool,
}

impl PlayerService {
    pub fn new(
        catalog: PlayerCatalog,
        controller: PlaybackController,
        local_resolver: Arc<dyn LocalTrackResolver>,
        resolver_factory: Arc<dyn SourceResolverFactory>,
    ) -> Self {
        let state_store = PlaybackStateStore::from_catalog(catalog.clone());
        Self {
            catalog,
            state_store,
            controller,
            local_resolver,
            resolver_factory,
            source_resolvers: tokio::sync::RwLock::new(HashMap::new()),
            materializer: SourceMaterializer,
            state_writer_started: AtomicBool::new(false),
        }
    }

    pub fn start_state_writer(&self) {
        if self.state_writer_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let state_store = self.state_store.clone();
        let controller = self.controller.clone();
        let mut events = controller.subscribe_events();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut dirty = false;
            let mut was_playing = false;
            loop {
                tokio::select! {
                    event = events.recv() => match event {
                        Ok(stellatune_audio::playback::PlaybackEvent::StateChanged(state)) => {
                            was_playing = state == stellatune_audio::playback::PlaybackState::Playing;
                            dirty = true;
                        },
                        Ok(stellatune_audio::playback::PlaybackEvent::TrackChanged { .. }
                            | stellatune_audio::playback::PlaybackEvent::PlaybackEnded { .. }
                            | stellatune_audio::playback::PlaybackEvent::Position { .. }) => {
                            dirty = true;
                        },
                        Ok(_) => {},
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            dirty = true;
                        },
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    },
                    _ = interval.tick() => {
                        if !dirty {
                            continue;
                        }
                        if let Ok(snapshot) = controller.snapshot().await
                            && state_store.save_runtime_state(snapshot, was_playing).await.is_ok()
                        {
                            dirty = false;
                        }
                    },
                }
            }
        });
    }

    pub async fn register_resolver(
        &self,
        source: SourceInstanceId,
        resolver: Arc<dyn SourceResolver>,
    ) -> Result<(), PlayerServiceError> {
        let entry = self.catalog.source(source).await?;
        if !matches!(entry.binding, SourceBinding::Plugin { .. }) {
            return Err(PlayerServiceError::CatalogBindingMismatch);
        }
        self.source_resolvers.write().await.insert(source, resolver);
        Ok(())
    }

    pub async fn ensure_track(
        &self,
        input: ProviderTrackIdentityInput,
    ) -> Result<TrackId, PlayerServiceError> {
        self.catalog.ensure_track(input.try_into()?).await
    }

    pub async fn ensure_local_track(
        &self,
        library_track_id: i64,
    ) -> Result<TrackId, PlayerServiceError> {
        let source = self.catalog.ensure_local_source().await?;
        self.catalog
            .ensure_local_track(source, library_track_id)
            .await
    }

    pub async fn ensure_plugin_source(
        &self,
        provider_id: ProviderId,
        spec: SourceResolverSpec,
        resolver: Arc<dyn SourceResolver>,
    ) -> Result<SourceInstanceId, PlayerServiceError> {
        let source = self.catalog.ensure_plugin_source(provider_id, spec).await?;
        self.register_resolver(source, resolver).await?;
        Ok(source)
    }

    pub async fn switch_track(
        &self,
        track_id: TrackId,
        options: SwitchOptions,
    ) -> Result<PlaybackItemId, PlayerServiceError> {
        let item_id = self.state_store.enqueue(track_id).await?;
        let item = self.materialize_item(item_id, track_id).await?;
        self.controller.switch(item, options).await?;
        let snapshot = self.controller.snapshot().await?;
        self.state_store
            .save_runtime_state(snapshot, options.autoplay)
            .await?;
        Ok(item_id)
    }

    pub async fn queue_next(
        &self,
        track_id: TrackId,
    ) -> Result<PlaybackItemId, PlayerServiceError> {
        let item_id = self.state_store.enqueue(track_id).await?;
        let item = self.materialize_item(item_id, track_id).await?;
        self.controller.queue_next(item).await?;
        Ok(item_id)
    }

    pub async fn restore(&self) -> Result<PlaybackStateRecord, PlayerServiceError> {
        self.restore_with_policy(false).await
    }

    pub async fn restore_with_policy(
        &self,
        resume_playing_on_launch: bool,
    ) -> Result<PlaybackStateRecord, PlayerServiceError> {
        let state = self.state_store.load().await?;
        let Some(current_id) = state.current_item_id else {
            return Ok(state);
        };
        let current_index = state
            .queue
            .iter()
            .position(|entry| entry.item_id == current_id)
            .ok_or(PlayerServiceError::PlaybackStateInvariant)?;
        let current = &state.queue[current_index];
        let item = self
            .materialize_item(current.item_id, current.track_id)
            .await?;
        let capabilities = item.source.descriptor().capabilities;
        self.controller
            .switch(
                item,
                SwitchOptions {
                    autoplay: false,
                    transition: stellatune_audio::playback::SwitchTransition::ImmediateWithDeClick,
                },
            )
            .await?;
        if state.position_ms > 0 && capabilities.byte_seekable && !capabilities.live {
            self.controller
                .seek(stellatune_audio_core::MediaTime::from_millis(
                    state.position_ms,
                ))
                .await?;
        }
        if let Some(next) = state.queue.get(current_index + 1) {
            let next_item = self.materialize_item(next.item_id, next.track_id).await?;
            self.controller.queue_next(next_item).await?;
        }
        if resume_playing_on_launch && state.was_playing {
            self.controller.play().await?;
        }
        Ok(state)
    }

    pub async fn local_path_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<Option<PathBuf>, PlayerServiceError> {
        let track_id = self.state_store.track_for_item(item_id).await?;
        let track = self.catalog.track(track_id).await?;
        match track.origin {
            TrackOrigin::LocalLibrary { library_track_id } => self
                .local_resolver
                .resolve_path(library_track_id)
                .await
                .map(Some),
            TrackOrigin::Provider(_) => Ok(None),
        }
    }

    pub async fn track_id_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<TrackId, PlayerServiceError> {
        self.state_store.track_for_item(item_id).await
    }

    pub async fn local_library_track_id_for_item(
        &self,
        item_id: PlaybackItemId,
    ) -> Result<Option<i64>, PlayerServiceError> {
        let track_id = self.state_store.track_for_item(item_id).await?;
        let track = self.catalog.track(track_id).await?;
        Ok(match track.origin {
            TrackOrigin::LocalLibrary { library_track_id } => Some(library_track_id),
            TrackOrigin::Provider(_) => None,
        })
    }

    async fn materialize_item(
        &self,
        item_id: PlaybackItemId,
        track_id: TrackId,
    ) -> Result<PlaybackItem, PlayerServiceError> {
        let track = self.catalog.track(track_id).await?;
        if track.tombstoned {
            return Err(PlayerServiceError::TrackUnavailable(track_id));
        }
        let source = self.catalog.source(track.source).await?;
        if source.tombstoned {
            return Err(PlayerServiceError::SourceUnavailable(track.source));
        }
        let (spec, required_decoder) = match track.origin {
            TrackOrigin::LocalLibrary { library_track_id } => {
                let path = self.local_resolver.resolve_path(library_track_id).await?;
                (
                    ResolvedSourceSpec::File {
                        path,
                        media: MediaHints::default(),
                    },
                    None,
                )
            },
            TrackOrigin::Provider(key) => {
                let resolver = self.resolver_for_source(&source).await?;
                (
                    resolver.resolve(&source, &key).await?,
                    resolver.required_decoder(),
                )
            },
        };
        Ok(PlaybackItem {
            id: item_id,
            source: self.materializer.materialize(spec)?,
            required_decoder,
        })
    }

    async fn resolver_for_source(
        &self,
        source: &SourceCatalogEntry,
    ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError> {
        if let Some(resolver) = self.source_resolvers.read().await.get(&source.id).cloned() {
            return Ok(resolver);
        }
        let spec = source
            .resolver
            .as_ref()
            .ok_or(PlayerServiceError::ResolverUnavailable(source.id))?;
        let resolver = self.resolver_factory.create(spec)?;
        self.source_resolvers
            .write()
            .await
            .insert(source.id, Arc::clone(&resolver));
        Ok(resolver)
    }
}

#[derive(Debug, Error)]
pub enum PlayerServiceError {
    #[error("playback storage schema is not the current hard-switch schema")]
    IncompatiblePlaybackSchema,
    #[error("invalid {identity} value {value}")]
    InvalidIdentity { identity: &'static str, value: u64 },
    #[error("provider id is empty or too long")]
    InvalidProviderId,
    #[error("provider track key is invalid")]
    InvalidProviderTrackKey,
    #[error("invalid resolved source: {0}")]
    InvalidSourceSpec(String),
    #[error("source catalog and track origin do not match")]
    CatalogBindingMismatch,
    #[error("source {0:?} was not found")]
    SourceNotFound(SourceInstanceId),
    #[error("track {0:?} was not found")]
    TrackNotFound(TrackId),
    #[error("playback item {0:?} was not found")]
    PlaybackItemNotFound(PlaybackItemId),
    #[error("source {0:?} is unavailable")]
    SourceUnavailable(SourceInstanceId),
    #[error("track {0:?} is unavailable")]
    TrackUnavailable(TrackId),
    #[error("resolver for source {0:?} is unavailable")]
    ResolverUnavailable(SourceInstanceId),
    #[error("playback state violates queue/current invariants")]
    PlaybackStateInvariant,
    #[error("source materialization failed: {0}")]
    Materialize(String),
    #[error("source resolution failed: {0}")]
    Resolve(String),
    #[error("local library resolution failed: {0}")]
    LocalLibrary(String),
    #[error("local library track {0} was not found")]
    LocalTrackNotFound(i64),
    #[error(transparent)]
    Storage(#[from] sqlx::Error),
    #[error(transparent)]
    Control(#[from] stellatune_audio_core::PlaybackControlError),
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use stellatune_audio::planner::StageRegistrySnapshot;
    use stellatune_audio::playback::{PlaybackRuntime, PlaybackRuntimeConfig};
    use stellatune_audio_core::{
        AudioBlock, AudioFormat, DecodeError, DecodeStatus, DecodedStreamInfo, DecoderDescriptor,
        DecoderFactory, DecoderSeekStatus, FactoryError, MediaHints, OutputCompatibilityKey,
        SeekResult, SinkClockSnapshot, SinkError, SinkFactory, SinkStage, SinkWriteResult,
        SinkWriteState, StageId,
    };

    use super::{
        LocalTrackResolver, MAX_PERSISTED_QUEUE_ITEMS, PlayerCatalog, PlayerService,
        PlayerServiceError, ProviderId, ProviderTrackIdentity, ProviderTrackKey,
        ResolvedSourceSpec, SourceCatalogEntry, SourceInstanceId, SourceResolver,
        SourceResolverFactory, SourceResolverSpec,
    };

    fn resolver_spec(provider: &str) -> SourceResolverSpec {
        SourceResolverSpec::new(provider, "source", "{}").unwrap()
    }

    struct UnusedLocalResolver;

    #[async_trait]
    impl LocalTrackResolver for UnusedLocalResolver {
        async fn resolve_path(
            &self,
            _library_track_id: i64,
        ) -> Result<PathBuf, PlayerServiceError> {
            Err(PlayerServiceError::LocalTrackNotFound(0))
        }
    }

    struct FileLocalResolver {
        path: PathBuf,
        resolves: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl LocalTrackResolver for FileLocalResolver {
        async fn resolve_path(
            &self,
            _library_track_id: i64,
        ) -> Result<PathBuf, PlayerServiceError> {
            self.resolves.fetch_add(1, Ordering::SeqCst);
            Ok(self.path.clone())
        }
    }

    struct CountingResolverFactory {
        creates: Arc<AtomicUsize>,
        resolves: Arc<AtomicUsize>,
    }

    impl SourceResolverFactory for CountingResolverFactory {
        fn create(
            &self,
            _spec: &SourceResolverSpec,
        ) -> Result<Arc<dyn SourceResolver>, PlayerServiceError> {
            self.creates.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(CountingResolver {
                resolves: Arc::clone(&self.resolves),
            }))
        }
    }

    struct CountingResolver {
        resolves: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl SourceResolver for CountingResolver {
        async fn resolve(
            &self,
            _source: &SourceCatalogEntry,
            _key: &ProviderTrackKey,
        ) -> Result<ResolvedSourceSpec, PlayerServiceError> {
            self.resolves.fetch_add(1, Ordering::SeqCst);
            super::SourceResolutionInput::Http {
                url: "https://temporary.invalid/audio?ephemeral=locator-secret".to_owned(),
                headers: std::collections::BTreeMap::from([(
                    "Authorization".to_owned(),
                    "Bearer ephemeral-header-secret".to_owned(),
                )]),
                media: super::MediaHintsInput {
                    extension: Some("mp3".to_owned()),
                    ..Default::default()
                },
                seekable: true,
                live: false,
            }
            .try_into()
        }
    }

    struct TestDecoderFactory {
        descriptor: DecoderDescriptor,
    }

    impl TestDecoderFactory {
        fn new() -> Self {
            Self {
                descriptor: DecoderDescriptor {
                    id: StageId::new("test.restart-decoder").unwrap(),
                    priority: 1,
                    extensions: vec!["bin".to_owned()],
                    mime_types: Vec::new(),
                },
            }
        }
    }

    impl DecoderFactory for TestDecoderFactory {
        fn descriptor(&self) -> &DecoderDescriptor {
            &self.descriptor
        }

        fn create(&self) -> Result<Box<dyn stellatune_audio_core::DecoderStage>, FactoryError> {
            Ok(Box::new(TestDecoder {
                total: 0,
                remaining: 0,
            }))
        }
    }

    struct TestDecoder {
        total: u64,
        remaining: u64,
    }

    impl stellatune_audio_core::DecoderStage for TestDecoder {
        fn open(
            &mut self,
            mut source: Box<dyn stellatune_audio_core::EncodedSource>,
            _hints: &MediaHints,
        ) -> Result<DecodedStreamInfo, DecodeError> {
            let mut header = [0_u8; 2];
            source.read_exact(&mut header).map_err(DecodeError::Io)?;
            self.total = u64::from(header[0]);
            self.remaining = self.total;
            Ok(DecodedStreamInfo {
                format: AudioFormat {
                    sample_rate: 1_000,
                    channels: 1,
                    channel_mask: None,
                },
                duration_frames: Some(self.total),
                gapless_trim: None,
            })
        }

        fn decode(&mut self, output: &mut AudioBlock) -> Result<DecodeStatus, DecodeError> {
            if self.remaining == 0 {
                return Ok(DecodeStatus::EndOfStream);
            }
            let frames = self.remaining.min(10) as usize;
            output.samples.resize(frames, 1.0);
            self.remaining -= frames as u64;
            Ok(DecodeStatus::Produced { frames })
        }

        fn start_seek(&mut self, target_frame: u64) -> Result<DecoderSeekStatus, DecodeError> {
            let actual = target_frame.min(self.total);
            self.remaining = self.total.saturating_sub(actual);
            Ok(DecoderSeekStatus::Complete(SeekResult {
                actual_frame: actual,
            }))
        }

        fn continue_seek(&mut self) -> Result<DecoderSeekStatus, DecodeError> {
            Err(DecodeError::Unsupported)
        }

        fn reset(&mut self) {}
    }

    struct UnusedSinkFactory {
        id: StageId,
    }

    impl SinkFactory for UnusedSinkFactory {
        fn id(&self) -> &StageId {
            &self.id
        }

        fn compatibility_key(
            &self,
            format: AudioFormat,
        ) -> Result<OutputCompatibilityKey, FactoryError> {
            Ok(OutputCompatibilityKey {
                backend_id: "unused".to_owned(),
                device_id: None,
                sample_rate: format.sample_rate,
                channels: format.channels,
                route_revision: 0,
            })
        }

        fn create(&self) -> Result<Box<dyn SinkStage>, FactoryError> {
            Ok(Box::new(TestSink { consumed: 0 }))
        }
    }

    struct TestSink {
        consumed: u64,
    }

    impl SinkStage for TestSink {
        fn open(&mut self, _format: AudioFormat) -> Result<(), SinkError> {
            Ok(())
        }

        fn write(&mut self, block: &AudioBlock) -> Result<SinkWriteResult, SinkError> {
            self.consumed = self.consumed.saturating_add(block.frames() as u64);
            Ok(SinkWriteResult {
                consumed_frames: block.frames(),
                state: SinkWriteState::Ready,
            })
        }

        fn pause(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn resume(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn drain(&mut self) -> Result<(), SinkError> {
            Ok(())
        }
        fn discard(&mut self) -> Result<(), SinkError> {
            self.consumed = 0;
            Ok(())
        }
        fn clock_snapshot(&self) -> SinkClockSnapshot {
            SinkClockSnapshot {
                consumed_frames: self.consumed,
                buffered_frames: 0,
                epoch: 0,
            }
        }
        fn close(&mut self) {}
    }

    fn test_runtime() -> PlaybackRuntime {
        PlaybackRuntime::start(PlaybackRuntimeConfig::new(StageRegistrySnapshot {
            decoders: vec![Arc::new(TestDecoderFactory::new())],
            transforms: Vec::new(),
            sink: Arc::new(UnusedSinkFactory {
                id: StageId::new("test.restart-sink").unwrap(),
            }),
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn catalog_bootstrap_allocates_stable_typed_ids() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("player.sqlite");
        let catalog = PlayerCatalog::open(&path).await.unwrap();
        let local_source = catalog.ensure_local_source().await.unwrap();
        let local_track = catalog.ensure_local_track(local_source, 42).await.unwrap();
        let repeated = catalog.ensure_local_track(local_source, 42).await.unwrap();
        assert_eq!(local_track, repeated);

        let first_item = catalog.enqueue(local_track).await.unwrap();
        let second_item = catalog.enqueue(local_track).await.unwrap();
        assert_ne!(first_item, second_item);
        drop(catalog);

        let reopened = PlayerCatalog::open(&path).await.unwrap();
        assert_eq!(reopened.ensure_local_source().await.unwrap(), local_source);
        assert_eq!(
            reopened.ensure_local_track(local_source, 42).await.unwrap(),
            local_track
        );
        let third_item = reopened.enqueue(local_track).await.unwrap();
        assert!(third_item.get() > second_item.get());
    }

    #[tokio::test]
    async fn persisted_queue_is_bounded_and_tombstones_never_reuse_ids() {
        let directory = tempfile::tempdir().unwrap();
        let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
            .await
            .unwrap();
        let source = catalog.ensure_local_source().await.unwrap();
        let track = catalog.ensure_local_track(source, 77).await.unwrap();
        let mut latest = None;
        for _ in 0..(MAX_PERSISTED_QUEUE_ITEMS + 8) {
            latest = Some(catalog.enqueue(track).await.unwrap());
        }
        let state = catalog.load_state().await.unwrap();
        assert_eq!(state.queue.len() as i64, MAX_PERSISTED_QUEUE_ITEMS);
        assert_eq!(state.queue.last().map(|entry| entry.item_id), latest);

        catalog.tombstone_track(track).await.unwrap();
        assert_eq!(
            catalog.ensure_local_track(source, 77).await.unwrap(),
            track,
            "a tombstoned identity must not be reassigned"
        );
        assert!(matches!(
            catalog.enqueue(track).await,
            Err(PlayerServiceError::TrackUnavailable(id)) if id == track
        ));
        catalog.tombstone_source(source).await.unwrap();
        assert!(catalog.source(source).await.unwrap().tombstoned);
        assert!(catalog.track(track).await.unwrap().tombstoned);
    }

    #[tokio::test]
    async fn provider_keys_are_scoped_by_source_instance() {
        let directory = tempfile::tempdir().unwrap();
        let catalog = PlayerCatalog::open(directory.path().join("player.sqlite"))
            .await
            .unwrap();
        let source_a = catalog
            .ensure_plugin_source(
                ProviderId::new("provider-a").unwrap(),
                resolver_spec("provider-a"),
            )
            .await
            .unwrap();
        let source_b = catalog
            .ensure_plugin_source(
                ProviderId::new("provider-b").unwrap(),
                resolver_spec("provider-b"),
            )
            .await
            .unwrap();
        let key = ProviderTrackKey::Text("same-key".to_owned());
        let track_a = catalog
            .ensure_track(ProviderTrackIdentity {
                source_instance_id: source_a,
                provider_key: key.clone(),
            })
            .await
            .unwrap();
        let track_a_again = catalog
            .ensure_track(ProviderTrackIdentity {
                source_instance_id: source_a,
                provider_key: key.clone(),
            })
            .await
            .unwrap();
        let track_b = catalog
            .ensure_track(ProviderTrackIdentity {
                source_instance_id: source_b,
                provider_key: key,
            })
            .await
            .unwrap();
        assert_eq!(track_a, track_a_again);
        assert_ne!(track_a, track_b);
    }

    #[tokio::test]
    async fn mismatched_or_partial_player_schema_is_rejected_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("player.sqlite");
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}?mode=rwc", path.display()))
            .await
            .unwrap();
        sqlx::query("CREATE TABLE playback_queue(item_id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        drop(pool);

        assert!(matches!(
            PlayerCatalog::open(&path).await,
            Err(PlayerServiceError::IncompatiblePlaybackSchema)
        ));
        let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}", path.display()))
            .await
            .unwrap();
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='player_schema_meta'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn restart_recreates_provider_resolver_without_persisting_temporary_locator() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("player.sqlite");
        let catalog = PlayerCatalog::open(&path).await.unwrap();
        let source = catalog
            .ensure_plugin_source(
                ProviderId::new("provider-restart").unwrap(),
                resolver_spec("plugin-restart"),
            )
            .await
            .unwrap();
        let track = catalog
            .ensure_track(ProviderTrackIdentity {
                source_instance_id: source,
                provider_key: ProviderTrackKey::Text("stable-provider-key".to_owned()),
            })
            .await
            .unwrap();
        let item = catalog.enqueue(track).await.unwrap();
        drop(catalog);

        let reopened = PlayerCatalog::open(&path).await.unwrap();
        let creates = Arc::new(AtomicUsize::new(0));
        let resolves = Arc::new(AtomicUsize::new(0));
        let runtime = PlaybackRuntime::start(PlaybackRuntimeConfig::new(StageRegistrySnapshot {
            decoders: Vec::new(),
            transforms: Vec::new(),
            sink: Arc::new(UnusedSinkFactory {
                id: StageId::new("test.unused-sink").unwrap(),
            }),
        }))
        .unwrap();
        let service = PlayerService::new(
            reopened.clone(),
            runtime.controller(),
            Arc::new(UnusedLocalResolver),
            Arc::new(CountingResolverFactory {
                creates: Arc::clone(&creates),
                resolves: Arc::clone(&resolves),
            }),
        );
        let materialized = service.materialize_item(item, track).await.unwrap();
        assert_eq!(materialized.id, item);
        assert_eq!(creates.load(Ordering::SeqCst), 1);
        assert_eq!(resolves.load(Ordering::SeqCst), 1);

        let source_entry = reopened.source(source).await.unwrap();
        let persisted = source_entry.resolver.unwrap();
        assert!(!persisted.config_json.contains("locator-secret"));
        assert!(!persisted.config_json.contains("header-secret"));
        let schema = sqlx::query_scalar::<_, String>(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='source_catalog'",
        )
        .fetch_one(&reopened.pool)
        .await
        .unwrap();
        assert!(!schema.contains("url"));
        assert!(!schema.contains("headers"));
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn restart_restores_local_item_and_sink_consumed_position_as_paused() {
        let directory = tempfile::tempdir().unwrap();
        let database_path = directory.path().join("player.sqlite");
        let media_path = directory.path().join("fixture.bin");
        std::fs::write(&media_path, [100_u8, 100_u8]).unwrap();
        let local_resolves = Arc::new(AtomicUsize::new(0));

        let catalog = PlayerCatalog::open(&database_path).await.unwrap();
        let runtime = test_runtime();
        let service = PlayerService::new(
            catalog.clone(),
            runtime.controller(),
            Arc::new(FileLocalResolver {
                path: media_path.clone(),
                resolves: Arc::clone(&local_resolves),
            }),
            Arc::new(CountingResolverFactory {
                creates: Arc::new(AtomicUsize::new(0)),
                resolves: Arc::new(AtomicUsize::new(0)),
            }),
        );
        let track = service.ensure_local_track(7).await.unwrap();
        let item = service
            .switch_track(
                track,
                stellatune_audio::playback::SwitchOptions {
                    autoplay: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        service
            .controller
            .seek(stellatune_audio_core::MediaTime::from_millis(40))
            .await
            .unwrap();
        let snapshot = service.controller.snapshot().await.unwrap();
        service
            .state_store
            .save_runtime_state(snapshot, false)
            .await
            .unwrap();
        runtime.shutdown().await.unwrap();
        drop(service);
        drop(catalog);

        let reopened = PlayerCatalog::open(&database_path).await.unwrap();
        let restarted_runtime = test_runtime();
        let restarted = PlayerService::new(
            reopened,
            restarted_runtime.controller(),
            Arc::new(FileLocalResolver {
                path: media_path,
                resolves: Arc::clone(&local_resolves),
            }),
            Arc::new(CountingResolverFactory {
                creates: Arc::new(AtomicUsize::new(0)),
                resolves: Arc::new(AtomicUsize::new(0)),
            }),
        );
        let restored = restarted.restore().await.unwrap();
        assert_eq!(restored.current_item_id, Some(item));
        assert_eq!(restored.position_ms, 40);
        assert_eq!(restored.queue[0].item_id, item);
        assert_eq!(restored.queue[0].track_id, track);
        let snapshot = restarted.controller.snapshot().await.unwrap();
        assert_eq!(snapshot.current_item_id, Some(item));
        assert_eq!(snapshot.consumed_position.as_millis(), 40);
        assert_eq!(
            snapshot.state,
            stellatune_audio::playback::PlaybackState::Ready
        );
        assert_eq!(local_resolves.load(Ordering::SeqCst), 2);
        restarted_runtime.shutdown().await.unwrap();
    }

    #[test]
    fn identity_types_reject_zero_and_sqlite_overflow() {
        assert!(SourceInstanceId::new(0).is_err());
        assert!(SourceInstanceId::new(i64::MAX as u64 + 1).is_err());
    }
}
