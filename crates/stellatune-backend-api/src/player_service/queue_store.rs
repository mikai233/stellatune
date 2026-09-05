//! Transactional queue edits preserve the identities of retained occurrences.

use super::{
    catalog::PlayerCatalog,
    catalog_batch::{BATCH_SIZE, validate_tracks},
    error::PlayerServiceError,
    identity::TrackId,
    state::{PlaybackQueueRecord, RepeatMode},
};
use sqlx::{QueryBuilder, Row, Sqlite, SqliteConnection};
use stellatune_audio_core::playback::PlaybackItemId;

impl PlayerCatalog {
    pub async fn replace_items(
        &self,
        tracks: &[TrackId],
    ) -> Result<Vec<PlaybackQueueRecord>, PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        validate_tracks(&mut tx, tracks).await?;
        sqlx::query("UPDATE playback_state SET current_item_id=NULL,position_ms=0,was_playing=0 WHERE singleton=1")
            .execute(&mut *tx).await?;
        sqlx::query("DELETE FROM playback_queue")
            .execute(&mut *tx)
            .await?;
        let items = insert_items(&mut tx, tracks, 0).await?;
        tx.commit().await?;
        Ok(items)
    }

    pub async fn append_items(
        &self,
        tracks: &[TrackId],
    ) -> Result<Vec<PlaybackQueueRecord>, PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        validate_tracks(&mut tx, tracks).await?;
        let base: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(queue_position)+1,0) FROM playback_queue")
                .fetch_one(&mut *tx)
                .await?;
        let items = insert_items(&mut tx, tracks, base).await?;
        tx.commit().await?;
        Ok(items)
    }

    pub async fn remove_items(&self, items: &[PlaybackItemId]) -> Result<(), PlayerServiceError> {
        let mut tx = self.pool.begin().await?;
        for item in items {
            sqlx::query("UPDATE playback_state SET current_item_id=NULL,position_ms=0 WHERE current_item_id=?")
                .bind(item.get() as i64).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM playback_queue WHERE item_id=?")
                .bind(item.get() as i64)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn save_queue_mode(
        &self,
        repeat: RepeatMode,
        shuffle: bool,
    ) -> Result<(), PlayerServiceError> {
        let mode = match repeat {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        };
        sqlx::query("UPDATE playback_state SET repeat_mode=?,shuffle_enabled=? WHERE singleton=1")
            .bind(mode)
            .bind(shuffle)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

async fn insert_items(
    connection: &mut SqliteConnection,
    tracks: &[TrackId],
    base: i64,
) -> Result<Vec<PlaybackQueueRecord>, PlayerServiceError> {
    let mut items = Vec::with_capacity(tracks.len());
    for (batch, chunk) in tracks.chunks(BATCH_SIZE).enumerate() {
        let mut query =
            QueryBuilder::<Sqlite>::new("INSERT INTO playback_queue(track_id,queue_position) ");
        query.push_values(chunk.iter().enumerate(), |mut row, (offset, track)| {
            row.push_bind(track.as_i64())
                .push_bind(base + (batch * BATCH_SIZE + offset) as i64);
        });
        query.push(" RETURNING item_id,track_id,queue_position");
        let mut rows = query.build().fetch_all(&mut *connection).await?;
        // SQLite does not guarantee RETURNING row order.
        rows.sort_by_key(|row| row.get::<i64, _>("queue_position"));
        for row in rows {
            items.push(PlaybackQueueRecord {
                item_id: PlaybackItemId::new(row.get::<i64, _>("item_id") as u64)
                    .ok_or(PlayerServiceError::PlaybackStateInvariant)?,
                track_id: TrackId::new(row.get::<i64, _>("track_id") as u64)?,
            });
        }
    }
    Ok(items)
}
