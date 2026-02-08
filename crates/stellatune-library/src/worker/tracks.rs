use anyhow::{Context, Result};
use sqlx::Row;
use sqlx::SqlitePool;
use std::path::Path;

#[derive(Debug, sqlx::FromRow)]
pub(super) struct TrackLiteRow {
    pub(super) id: i64,
    pub(super) path: String,
    pub(super) title: Option<String>,
    pub(super) artist: Option<String>,
    pub(super) album: Option<String>,
    pub(super) duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TrackFingerprint {
    pub(super) mtime_ms: i64,
    pub(super) size_bytes: i64,
    pub(super) meta_scanned_ms: i64,
}

pub(super) struct UpsertTrackInput<'a> {
    pub(super) path: &'a str,
    pub(super) ext: &'a str,
    pub(super) mtime_ms: i64,
    pub(super) size_bytes: i64,
    pub(super) title: Option<&'a str>,
    pub(super) artist: Option<&'a str>,
    pub(super) album: Option<&'a str>,
    pub(super) duration_ms: Option<i64>,
    pub(super) meta_scanned_ms: i64,
    pub(super) path_norm: &'a str,
    pub(super) dir_norm: &'a str,
}

pub(super) async fn select_track_fingerprint(
    pool: &SqlitePool,
    path: &str,
) -> Result<Option<TrackFingerprint>> {
    let row = sqlx::query!(
        "SELECT id, mtime_ms, size_bytes, meta_scanned_ms FROM tracks WHERE path=?1",
        path
    )
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else { return Ok(None) };
    let _ = r.id.context("tracks.id is null")?;
    Ok(Some(TrackFingerprint {
        mtime_ms: r.mtime_ms,
        size_bytes: r.size_bytes,
        meta_scanned_ms: r.meta_scanned_ms,
    }))
}

pub(super) async fn select_track_fingerprint_by_path_norm(
    pool: &SqlitePool,
    path_norm: &str,
) -> Result<Option<TrackFingerprint>> {
    let row = sqlx::query(
        "SELECT id, mtime_ms, size_bytes, meta_scanned_ms FROM tracks WHERE path_norm=?1 LIMIT 1",
    )
    .bind(path_norm)
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else { return Ok(None) };
    let id: i64 = r.try_get("id")?;
    let _ = id;
    Ok(Some(TrackFingerprint {
        mtime_ms: r.try_get("mtime_ms")?,
        size_bytes: r.try_get("size_bytes")?,
        meta_scanned_ms: r.try_get("meta_scanned_ms")?,
    }))
}

pub(super) async fn delete_track_by_path_norm(
    pool: &SqlitePool,
    cover_dir: &Path,
    path_norm: &str,
) -> Result<u64> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE path_norm=?1")
        .bind(path_norm)
        .fetch_all(pool)
        .await?;

    if ids.is_empty() {
        return Ok(0);
    }

    let deleted = sqlx::query("DELETE FROM tracks WHERE path_norm=?1")
        .bind(path_norm)
        .execute(pool)
        .await?
        .rows_affected();

    // Best-effort cover cleanup.
    for id in ids {
        let final_path = cover_dir.join(id.to_string());
        let tmp_path = cover_dir.join(format!("{id}.tmp"));
        let _ = std::fs::remove_file(final_path);
        let _ = std::fs::remove_file(tmp_path);
    }

    Ok(deleted)
}

pub(super) async fn upsert_track(pool: &SqlitePool, input: UpsertTrackInput<'_>) -> Result<i64> {
    sqlx::query!(
        r#"
        INSERT INTO tracks(path, ext, mtime_ms, size_bytes, title, artist, album, duration_ms, meta_scanned_ms, path_norm, dir_norm)
        VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(path) DO UPDATE SET
            ext=excluded.ext,
            mtime_ms=excluded.mtime_ms,
            size_bytes=excluded.size_bytes,
            title=COALESCE(excluded.title, title),
            artist=COALESCE(excluded.artist, artist),
            album=COALESCE(excluded.album, album),
            duration_ms=COALESCE(excluded.duration_ms, duration_ms),
            meta_scanned_ms=excluded.meta_scanned_ms,
            path_norm=excluded.path_norm,
            dir_norm=excluded.dir_norm
        "#,
        input.path,
        input.ext,
        input.mtime_ms,
        input.size_bytes,
        input.title,
        input.artist,
        input.album,
        input.duration_ms,
        input.meta_scanned_ms,
        input.path_norm,
        input.dir_norm
    )
    .execute(pool)
    .await?;

    let id: i64 = sqlx::query_scalar!("SELECT id FROM tracks WHERE path=?1", input.path)
        .fetch_one(pool)
        .await?
        .context("tracks.id is null")?;
    Ok(id)
}

pub(super) async fn upsert_track_by_path_norm(
    pool: &SqlitePool,
    input: UpsertTrackInput<'_>,
) -> Result<i64> {
    let existing_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM tracks WHERE path_norm=?1 LIMIT 1")
            .bind(input.path_norm)
            .fetch_optional(pool)
            .await?;

    if let Some(id) = existing_id {
        sqlx::query(
            r#"
            UPDATE tracks
            SET
              path=?1,
              ext=?2,
              mtime_ms=?3,
              size_bytes=?4,
              title=COALESCE(?5, title),
              artist=COALESCE(?6, artist),
              album=COALESCE(?7, album),
              duration_ms=COALESCE(?8, duration_ms),
              meta_scanned_ms=?9,
              path_norm=?10,
              dir_norm=?11
            WHERE id=?12
            "#,
        )
        .bind(input.path)
        .bind(input.ext)
        .bind(input.mtime_ms)
        .bind(input.size_bytes)
        .bind(input.title)
        .bind(input.artist)
        .bind(input.album)
        .bind(input.duration_ms)
        .bind(input.meta_scanned_ms)
        .bind(input.path_norm)
        .bind(input.dir_norm)
        .bind(id)
        .execute(pool)
        .await?;
        return Ok(id);
    }

    upsert_track(pool, input).await
}
