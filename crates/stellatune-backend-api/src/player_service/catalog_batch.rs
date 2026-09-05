//! Bulk catalog operations keep library-sized queues off the per-track SQL path.

use std::collections::HashMap;

use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection};

use super::{catalog::PlayerCatalog, error::PlayerServiceError, identity::TrackId};

// Keep statements below SQLite's bind limit, including builds using 999 variables.
pub(super) const BATCH_SIZE: usize = 400;

impl PlayerCatalog {
    /// Registers local identities atomically, preserving input order and duplicates.
    pub async fn ensure_local_tracks(
        &self,
        library_ids: &[i64],
    ) -> Result<Vec<TrackId>, PlayerServiceError> {
        if library_ids.iter().any(|id| *id <= 0) {
            return Err(PlayerServiceError::CatalogBindingMismatch);
        }
        if library_ids.is_empty() {
            return Ok(vec![]);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT OR IGNORE INTO source_catalog(binding_kind) VALUES('local')")
            .execute(&mut *tx)
            .await?;
        let source: i64 =
            sqlx::query_scalar("SELECT id FROM source_catalog WHERE binding_kind='local'")
                .fetch_one(&mut *tx)
                .await?;
        let mut identities = HashMap::new();
        for chunk in library_ids.chunks(BATCH_SIZE) {
            let mut insert = QueryBuilder::<Sqlite>::new(
                "INSERT OR IGNORE INTO track_catalog(source_id,origin_kind,local_library_id) ",
            );
            insert.push_values(chunk, |mut row, id| {
                row.push_bind(source).push("'local'").push_bind(*id);
            });
            insert.build().execute(&mut *tx).await?;
            let mut select = QueryBuilder::<Sqlite>::new(
                "SELECT id,local_library_id FROM track_catalog WHERE source_id=",
            );
            select.push_bind(source).push(" AND local_library_id IN (");
            let mut values = select.separated(",");
            for id in chunk {
                values.push_bind(*id);
            }
            select.push(")");
            for row in select.build().fetch_all(&mut *tx).await? {
                identities.insert(
                    row.get::<i64, _>("local_library_id"),
                    TrackId::new(row.get::<i64, _>("id") as u64)?,
                );
            }
        }
        tx.commit().await?;
        library_ids
            .iter()
            .map(|id| {
                identities
                    .get(id)
                    .copied()
                    .ok_or(PlayerServiceError::PlaybackStateInvariant)
            })
            .collect()
    }

    pub(super) async fn validate_tracks(
        &self,
        tracks: &[TrackId],
    ) -> Result<(), PlayerServiceError> {
        let mut connection = self.pool.acquire().await?;
        validate_tracks(&mut connection, tracks).await
    }

    /// Looks up catalog identities by track, independent of concurrent queue edits.
    pub(super) async fn local_library_ids(
        &self,
        tracks: &[TrackId],
    ) -> Result<HashMap<TrackId, i64>, PlayerServiceError> {
        let mut result = HashMap::new();
        for chunk in tracks.chunks(BATCH_SIZE) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT id,local_library_id FROM track_catalog WHERE origin_kind='local' AND id IN (",
            );
            let mut values = query.separated(",");
            for id in chunk {
                values.push_bind(id.as_i64());
            }
            query.push(")");
            for row in query.build().fetch_all(&self.pool).await? {
                result.insert(
                    TrackId::new(row.get::<i64, _>("id") as u64)?,
                    row.get("local_library_id"),
                );
            }
        }
        Ok(result)
    }
}

pub(super) async fn validate_tracks(
    connection: &mut SqliteConnection,
    tracks: &[TrackId],
) -> Result<(), PlayerServiceError> {
    for chunk in tracks.chunks(BATCH_SIZE) {
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT t.id,t.tombstoned,s.tombstoned AS source_tombstoned FROM track_catalog t JOIN source_catalog s ON s.id=t.source_id WHERE t.id IN (",
        );
        let mut values = query.separated(",");
        for id in chunk {
            values.push_bind(id.as_i64());
        }
        query.push(")");
        let rows = query.build().fetch_all(&mut *connection).await?;
        let available: HashMap<i64, bool> = rows
            .iter()
            .map(|row| {
                (
                    row.get("id"),
                    row.get::<i64, _>("tombstoned") == 0
                        && row.get::<i64, _>("source_tombstoned") == 0,
                )
            })
            .collect();
        for id in chunk {
            match available.get(&id.as_i64()) {
                None => return Err(PlayerServiceError::TrackNotFound(*id)),
                Some(false) => return Err(PlayerServiceError::TrackUnavailable(*id)),
                Some(true) => {},
            }
        }
    }
    Ok(())
}
