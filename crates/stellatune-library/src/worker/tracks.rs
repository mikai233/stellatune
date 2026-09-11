use anyhow::Result;
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
    pub(super) album_artist: Option<&'a str>,
    pub(super) disc_number: Option<i64>,
    pub(super) track_number: Option<i64>,
    pub(super) artists_json: &'a str,
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
        "SELECT mtime_ms, size_bytes, meta_scanned_ms FROM tracks WHERE path=?1",
        path
    )
    .fetch_optional(pool)
    .await?;

    let Some(r) = row else { return Ok(None) };
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
    // An unavailable tag (or failed inspection) must not erase known metadata.
    // Empty artist lists likewise mean no new artist information was obtained.
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks(path,ext,mtime_ms,size_bytes,title,artist,album,duration_ms,meta_scanned_ms,path_norm,dir_norm,album_artist,disc_number,track_number,artists_json)
        VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(path) DO UPDATE SET
        ext=excluded.ext,mtime_ms=excluded.mtime_ms,size_bytes=excluded.size_bytes,
        title=COALESCE(excluded.title,tracks.title),artist=COALESCE(excluded.artist,tracks.artist),album=COALESCE(excluded.album,tracks.album),duration_ms=COALESCE(excluded.duration_ms,tracks.duration_ms),
        meta_scanned_ms=excluded.meta_scanned_ms,path_norm=excluded.path_norm,dir_norm=excluded.dir_norm,
        album_artist=COALESCE(excluded.album_artist,tracks.album_artist),disc_number=COALESCE(excluded.disc_number,tracks.disc_number),track_number=COALESCE(excluded.track_number,tracks.track_number),artists_json=CASE WHEN json_array_length(excluded.artists_json)>0 THEN excluded.artists_json WHEN excluded.artist IS NOT NULL THEN json_array(excluded.artist) ELSE tracks.artists_json END RETURNING id")
        .bind(input.path).bind(input.ext).bind(input.mtime_ms).bind(input.size_bytes)
        .bind(input.title).bind(input.artist).bind(input.album).bind(input.duration_ms)
        .bind(input.meta_scanned_ms).bind(input.path_norm).bind(input.dir_norm)
        .bind(input.album_artist).bind(input.disc_number).bind(input.track_number).bind(input.artists_json)
        .fetch_one(pool).await?;
    Ok(id)
}

pub(super) async fn upsert_track_by_path_norm(
    pool: &SqlitePool,
    input: UpsertTrackInput<'_>,
) -> Result<i64> {
    let existing_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM tracks WHERE path_norm=? LIMIT 1")
            .bind(input.path_norm)
            .fetch_optional(pool)
            .await?;
    if let Some(id) = existing_id {
        sqlx::query("UPDATE tracks SET path=?1,ext=?2,mtime_ms=?3,size_bytes=?4,title=COALESCE(?5,title),artist=COALESCE(?6,artist),album=COALESCE(?7,album),duration_ms=COALESCE(?8,duration_ms),meta_scanned_ms=?9,path_norm=?10,dir_norm=?11,album_artist=COALESCE(?12,album_artist),disc_number=COALESCE(?13,disc_number),track_number=COALESCE(?14,track_number),artists_json=CASE WHEN json_array_length(?15)>0 THEN ?15 WHEN ?6 IS NOT NULL THEN json_array(?6) ELSE artists_json END WHERE id=?16")
            .bind(input.path).bind(input.ext).bind(input.mtime_ms).bind(input.size_bytes)
            .bind(input.title).bind(input.artist).bind(input.album).bind(input.duration_ms)
            .bind(input.meta_scanned_ms).bind(input.path_norm).bind(input.dir_norm)
            .bind(input.album_artist).bind(input.disc_number).bind(input.track_number).bind(input.artists_json).bind(id)
            .execute(pool).await?;
        return Ok(id);
    }
    upsert_track(pool, input).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> UpsertTrackInput<'static> {
        UpsertTrackInput {
            path: "D:/music/song.mp3",
            ext: "mp3",
            mtime_ms: 1,
            size_bytes: 100,
            title: None,
            artist: None,
            album: None,
            album_artist: None,
            disc_number: None,
            track_number: None,
            artists_json: "[]",
            duration_ms: None,
            meta_scanned_ms: 1,
            path_norm: "D:/music/song.mp3",
            dir_norm: "D:/music",
        }
    }

    #[tokio::test]
    async fn rescan_retains_missing_metadata_but_accepts_new_tags_for_both_paths() {
        for normalized in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let pool = crate::worker::db::init_db(&dir.path().join("library.db"))
                .await
                .unwrap();
            let id = upsert_track(
                &pool,
                UpsertTrackInput {
                    title: Some("Song"),
                    artist: Some("Artist"),
                    album: Some("Album"),
                    album_artist: Some("Album Artist"),
                    disc_number: Some(1),
                    track_number: Some(2),
                    artists_json: "[\"Artist\",\"Guest\"]",
                    duration_ms: Some(123000),
                    ..input()
                },
            )
            .await
            .unwrap();
            let snapshot = || {
                sqlx::query_scalar::<_, String>(
                "SELECT json_array(title,artist,album,album_artist,disc_number,track_number,artists_json,duration_ms) FROM tracks"
            ).fetch_one(&pool)
            };
            let before = snapshot().await.unwrap();
            let empty = UpsertTrackInput {
                meta_scanned_ms: 2,
                ..input()
            };
            let same_id = if normalized {
                upsert_track_by_path_norm(&pool, empty).await
            } else {
                upsert_track(&pool, empty).await
            }
            .unwrap();
            assert_eq!(same_id, id);
            assert_eq!(snapshot().await.unwrap(), before);
            let scanned: i64 = sqlx::query_scalar("SELECT meta_scanned_ms FROM tracks")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(scanned, 2);
            let artists: Vec<String> =
                sqlx::query_scalar("SELECT name FROM catalog_artists ORDER BY name")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert_eq!(artists, ["Album Artist", "Artist", "Guest"]);

            let updated = UpsertTrackInput {
                title: Some("New Song"),
                artist: Some("New Artist"),
                album_artist: Some("New Artist"),
                ..input()
            };
            if normalized {
                upsert_track_by_path_norm(&pool, updated).await.unwrap();
            } else {
                upsert_track(&pool, updated).await.unwrap();
            }
            let row = sqlx::query("SELECT title,album,artists_json FROM tracks")
                .fetch_one(&pool)
                .await
                .unwrap();
            assert_eq!(row.get::<String, _>("title"), "New Song");
            assert_eq!(row.get::<String, _>("album"), "Album");
            assert_eq!(row.get::<String, _>("artists_json"), "[\"New Artist\"]");
            let artists: Vec<String> =
                sqlx::query_scalar("SELECT name FROM catalog_artists ORDER BY name")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert_eq!(artists, ["New Artist"]);
        }
    }
}
