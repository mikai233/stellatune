use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Error, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{Connection, Row, SqliteConnection};

use crate::LyricsDoc;

use super::actor::LyricsServiceCore;
use super::parser::unix_now_ms;

const CACHE_TTL_MS: i64 = 30_i64 * 24 * 60 * 60 * 1000;

impl LyricsServiceCore {
    pub(super) fn cache_db_path(&self) -> Option<PathBuf> {
        self.cache_db_path
            .load_full()
            .map(|db_path| db_path.as_ref().clone())
    }

    pub(super) async fn load_doc_from_cache_db(&self, track_key: &str) -> Option<LyricsDoc> {
        let db_path = self.cache_db_path()?;
        match async {
            let mut conn = Self::open_cache_db(&db_path).await?;
            let row = sqlx::query(
                "SELECT doc_json, updated_at_ms FROM lyrics_cache WHERE track_key = ?1 LIMIT 1",
            )
            .bind(track_key)
            .fetch_optional(&mut conn)
            .await
            .context("query lyrics cache failed")?;
            let Some(row) = row else {
                return Ok(None);
            };
            let updated_at_ms: i64 = row
                .try_get("updated_at_ms")
                .context("lyrics cache missing updated_at_ms")?;
            let now_ms = unix_now_ms();
            if now_ms.saturating_sub(updated_at_ms) > CACHE_TTL_MS {
                sqlx::query("DELETE FROM lyrics_cache WHERE track_key = ?1")
                    .bind(track_key)
                    .execute(&mut conn)
                    .await
                    .context("delete stale lyrics cache row failed")?;
                return Ok(None);
            }
            let doc_json: String = row
                .try_get("doc_json")
                .context("lyrics cache missing doc_json")?;
            let doc: LyricsDoc =
                serde_json::from_str(&doc_json).context("parse lyrics cache doc_json failed")?;
            Ok::<_, Error>(Some(doc))
        }
        .await
        {
            Ok(doc) => doc,
            Err(err) => {
                tracing::warn!("load lyrics cache failed: {err}");
                None
            },
        }
    }

    pub(super) async fn persist_doc_to_cache_db(&self, doc: &LyricsDoc) -> Result<()> {
        let Some(db_path) = self.cache_db_path() else {
            return Ok(());
        };
        let track_key = doc.track_key.clone();
        let source = doc.source.clone();
        let is_synced = if doc.is_synced { 1_i64 } else { 0_i64 };
        let doc_json = serde_json::to_string(doc).context("serialize lyrics doc failed")?;
        let updated_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system time before unix epoch")?
            .as_millis() as i64;

        let mut conn = Self::open_cache_db(&db_path).await?;
        sqlx::query(
            "INSERT INTO lyrics_cache (track_key, source, is_synced, doc_json, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(track_key) DO UPDATE SET
               source = excluded.source,
               is_synced = excluded.is_synced,
               doc_json = excluded.doc_json,
               updated_at_ms = excluded.updated_at_ms",
        )
        .bind(track_key)
        .bind(source)
        .bind(is_synced)
        .bind(doc_json)
        .bind(updated_at_ms)
        .execute(&mut conn)
        .await
        .context("upsert lyrics cache failed")?;
        Ok(())
    }

    pub(super) async fn init_cache_db(path: &Path) -> Result<()> {
        let mut conn = Self::open_cache_db(path).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS lyrics_cache (
               track_key TEXT PRIMARY KEY NOT NULL,
               source TEXT NOT NULL,
               is_synced INTEGER NOT NULL,
               doc_json TEXT NOT NULL,
               updated_at_ms INTEGER NOT NULL
             )",
        )
        .execute(&mut conn)
        .await
        .context("create lyrics cache table failed")?;
        Ok(())
    }

    pub(super) async fn open_cache_db(path: &Path) -> Result<SqliteConnection> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let conn = SqliteConnection::connect_with(&options)
            .await
            .with_context(|| format!("open lyrics cache db failed: {}", path.display()))?;
        Ok(conn)
    }
}
