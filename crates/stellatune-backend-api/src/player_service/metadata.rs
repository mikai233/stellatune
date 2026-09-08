//! Durable presentation data for provider tracks, keyed by application identity.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, Sqlite};

use super::{
    catalog::PlayerCatalog, catalog_batch::BATCH_SIZE, error::PlayerServiceError,
    identity::TrackId, service::PlayerService,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrackCover {
    pub kind: TrackCoverKind,
    pub value: String,
    pub mime: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackCoverKind {
    Url,
    File,
    Data,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrackPresentation {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub cover: Option<TrackCover>,
}

#[derive(Debug, Clone)]
pub struct ProviderQueueMetadata {
    pub provider_id: String,
    pub provider_key: String,
    pub plugin_id: String,
    pub capability_id: String,
    pub presentation: Option<TrackPresentation>,
}

impl PlayerCatalog {
    // Additive table: existing track, queue and playback identity schemas remain intact.
    pub(super) async fn ensure_metadata_table(&self) -> Result<(), PlayerServiceError> {
        sqlx::query("CREATE TABLE IF NOT EXISTS track_presentation(\
            track_id INTEGER PRIMARY KEY REFERENCES track_catalog(id), presentation_json TEXT NOT NULL)")
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn store_track_presentations(
        &self,
        updates: &[(TrackId, TrackPresentation)],
    ) -> Result<(), PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        for (track, presentation) in updates {
            let json = serde_json::to_string(presentation)
                .map_err(|error| PlayerServiceError::InvalidSourceSpec(error.to_string()))?;
            sqlx::query(
                "INSERT INTO track_presentation(track_id,presentation_json) VALUES(?,?) \
                ON CONFLICT(track_id) DO UPDATE SET presentation_json=excluded.presentation_json",
            )
            .bind(track.as_i64())
            .bind(json)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn provider_queue_metadata(
        &self,
        tracks: &[TrackId],
    ) -> Result<HashMap<TrackId, ProviderQueueMetadata>, PlayerServiceError> {
        let mut result = HashMap::new();
        for chunk in tracks.chunks(BATCH_SIZE) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT t.id,t.provider_numeric,t.provider_text,s.provider_id,s.resolver_plugin_id,\
                 s.resolver_capability_id,m.presentation_json FROM track_catalog t \
                 JOIN source_catalog s ON s.id=t.source_id LEFT JOIN track_presentation m ON m.track_id=t.id \
                 WHERE s.binding_kind='plugin' AND t.id IN (",
            );
            let mut ids = query.separated(",");
            for track in chunk {
                ids.push_bind(track.as_i64());
            }
            query.push(")");
            for row in query.build().fetch_all(&self.pool).await? {
                let plugin_id: String = row.get("resolver_plugin_id");
                let capability_id: String = row.get("resolver_capability_id");
                let provider: String = row.get("provider_id");
                let prefix = format!("{plugin_id}::{capability_id}::");
                let provider_id = provider
                    .strip_prefix(&prefix)
                    .unwrap_or(&provider)
                    .to_owned();
                let provider_key = row
                    .get::<Option<i64>, _>("provider_numeric")
                    .map(|id| (id as u64).to_string())
                    .unwrap_or_else(|| row.get("provider_text"));
                let presentation = row
                    .get::<Option<String>, _>("presentation_json")
                    .map(|json| serde_json::from_str(&json))
                    .transpose()
                    .map_err(|error| PlayerServiceError::InvalidSourceSpec(error.to_string()))?;
                result.insert(
                    TrackId::new(row.get::<i64, _>("id") as u64)?,
                    ProviderQueueMetadata {
                        provider_id,
                        provider_key,
                        plugin_id,
                        capability_id,
                        presentation,
                    },
                );
            }
        }
        Ok(result)
    }
}

impl PlayerService {
    pub async fn queue_provider_metadata(
        &self,
        tracks: &[TrackId],
    ) -> Result<HashMap<TrackId, ProviderQueueMetadata>, PlayerServiceError> {
        self.catalog.provider_queue_metadata(tracks).await
    }

    pub async fn store_track_presentations(
        &self,
        updates: &[(TrackId, TrackPresentation)],
    ) -> Result<(), PlayerServiceError> {
        if updates.is_empty() {
            return Ok(());
        }
        self.catalog.store_track_presentations(updates).await?;
        self.notify_presentation_changed().await;
        Ok(())
    }
}
