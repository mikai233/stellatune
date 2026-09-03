use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use stellatune_audio::playback::event::PlaybackRuntimeSnapshot;
use stellatune_audio_core::PlaybackItemId;

use super::error::PlayerServiceError;
use super::identity::{
    ProviderId, ProviderTrackIdentity, ProviderTrackKey, SourceBinding, SourceInstanceId, TrackId,
};
use super::source::{SourceCatalogEntry, SourceResolverSpec, TrackCatalogEntry, TrackOrigin};
use super::state::{PlaybackQueueRecord, PlaybackStateRecord, RepeatMode};

const PLAYER_SCHEMA_VERSION: i64 = 1;
pub(super) const MAX_PERSISTED_QUEUE_ITEMS: i64 = 1_024;
const PLAYER_SCHEMA_FINGERPRINT: &str = "stellatune-player-v1-typed-catalog-state-20260901";
#[derive(Clone)]
pub struct PlayerCatalog {
    pub(super) pool: SqlitePool,
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
