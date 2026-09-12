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
    let row =
        sqlx::query("SELECT mtime_ms,size_bytes,meta_scanned_ms FROM audio_files WHERE path=?1 AND EXISTS(SELECT 1 FROM tracks WHERE file_id=audio_files.id)")
            .bind(path)
            .fetch_optional(pool)
            .await?;

    let Some(r) = row else { return Ok(None) };
    Ok(Some(TrackFingerprint {
        mtime_ms: r.try_get("mtime_ms")?,
        size_bytes: r.try_get("size_bytes")?,
        meta_scanned_ms: r.try_get("meta_scanned_ms")?,
    }))
}

pub(super) async fn select_track_fingerprint_by_path_norm(
    pool: &SqlitePool,
    path_norm: &str,
) -> Result<Option<TrackFingerprint>> {
    let row = sqlx::query(
        "SELECT id, mtime_ms, size_bytes, meta_scanned_ms FROM audio_files WHERE path_norm=?1 AND EXISTS(SELECT 1 FROM tracks WHERE file_id=audio_files.id)",
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

    let cover: Option<i64> =
        sqlx::query_scalar("SELECT cover_key FROM audio_files WHERE path_norm=?")
            .bind(path_norm)
            .fetch_optional(pool)
            .await?
            .flatten();
    sqlx::query("DELETE FROM audio_files WHERE path_norm=?")
        .bind(path_norm)
        .execute(pool)
        .await?;
    if ids.is_empty() {
        return Ok(0);
    }

    let deleted = ids.len() as u64;
    sqlx::query("DELETE FROM tracks WHERE path_norm=?1")
        .bind(path_norm)
        .execute(pool)
        .await?;

    // Best-effort cover cleanup.
    for id in ids.into_iter().chain(cover) {
        let final_path = cover_dir.join(id.to_string());
        let tmp_path = cover_dir.join(format!("{id}.tmp"));
        let _ = std::fs::remove_file(final_path);
        let _ = std::fs::remove_file(tmp_path);
    }

    Ok(deleted)
}

pub(super) async fn upsert_track(pool: &SqlitePool, input: UpsertTrackInput<'_>) -> Result<i64> {
    let artists = crate::artist_names::normalize_artists_json(input.artists_json, input.artist)?;
    let album_artists = crate::artist_names::normalize_artists_json("[]", input.album_artist)?;
    let file_id: i64 = sqlx::query_scalar("INSERT INTO audio_files(path,path_norm,ext,mtime_ms,size_bytes,meta_scanned_ms) VALUES(?,?,?,?,?,?) ON CONFLICT(path_norm) DO UPDATE SET path=excluded.path,ext=excluded.ext,mtime_ms=excluded.mtime_ms,size_bytes=excluded.size_bytes,meta_scanned_ms=excluded.meta_scanned_ms,total_frames=CASE WHEN audio_files.mtime_ms=excluded.mtime_ms AND audio_files.size_bytes=excluded.size_bytes THEN audio_files.total_frames END RETURNING id")
        .bind(input.path).bind(input.path_norm).bind(input.ext).bind(input.mtime_ms).bind(input.size_bytes).bind(input.meta_scanned_ms).fetch_one(pool).await?;
    let track_key = format!("file:{}", input.path_norm);
    // An unavailable tag (or failed inspection) must not erase known metadata.
    // Empty artist lists likewise mean no new artist information was obtained.
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks(path,ext,mtime_ms,size_bytes,title,artist,album,duration_ms,meta_scanned_ms,path_norm,dir_norm,album_artist,disc_number,track_number,artists_json,album_artists_json,artist_names_version,file_id,track_key)
        VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,1,?,?) ON CONFLICT(track_key) DO UPDATE SET
        ext=excluded.ext,mtime_ms=excluded.mtime_ms,size_bytes=excluded.size_bytes,
        title=COALESCE(excluded.title,tracks.title),artist=COALESCE(excluded.artist,tracks.artist),album=COALESCE(excluded.album,tracks.album),duration_ms=COALESCE(excluded.duration_ms,tracks.duration_ms),
        meta_scanned_ms=excluded.meta_scanned_ms,path_norm=excluded.path_norm,dir_norm=excluded.dir_norm,
        album_artist=COALESCE(excluded.album_artist,tracks.album_artist),disc_number=COALESCE(excluded.disc_number,tracks.disc_number),track_number=COALESCE(excluded.track_number,tracks.track_number),artists_json=CASE WHEN json_array_length(excluded.artists_json)>0 OR excluded.artist IS NOT NULL THEN excluded.artists_json ELSE tracks.artists_json END,
        album_artists_json=CASE WHEN excluded.album_artist IS NOT NULL THEN excluded.album_artists_json ELSE tracks.album_artists_json END,artist_names_version=1 RETURNING id")
        .bind(input.path).bind(input.ext).bind(input.mtime_ms).bind(input.size_bytes)
        .bind(input.title).bind(input.artist).bind(input.album).bind(input.duration_ms)
        .bind(input.meta_scanned_ms).bind(input.path_norm).bind(input.dir_norm)
        .bind(input.album_artist).bind(input.disc_number).bind(input.track_number).bind(artists).bind(album_artists).bind(file_id).bind(track_key)
        .fetch_one(pool).await?;
    sqlx::query("UPDATE audio_files SET cover_key=coalesce(cover_key,?) WHERE id=?")
        .bind(id)
        .bind(file_id)
        .execute(pool)
        .await?;
    Ok(id)
}

pub(crate) async fn attributes(
    pool: &SqlitePool,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, (i64, bool)>> {
    let mut out = std::collections::HashMap::new();
    for ids in ids.chunks(400) {
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT t.id,coalesce(f.cover_key,t.id) AS cover_key,t.start_frame IS NOT NULL AS is_segment FROM tracks t LEFT JOIN audio_files f ON f.id=t.file_id WHERE t.id IN (",
        );
        let mut bind = query.separated(",");
        for id in ids {
            bind.push_bind(id);
        }
        query.push(")");
        for row in query.build().fetch_all(pool).await? {
            out.insert(row.get("id"), (row.get("cover_key"), row.get("is_segment")));
        }
    }
    Ok(out)
}

pub(super) async fn decorate(pool: &SqlitePool, tracks: &mut [crate::TrackLite]) -> Result<()> {
    let attributes = attributes(pool, &tracks.iter().map(|t| t.id).collect::<Vec<_>>()).await?;
    for track in tracks {
        if let Some((cover, segment)) = attributes.get(&track.id) {
            track.cover_id = Some(*cover);
            track.is_segment = *segment;
        }
    }
    Ok(())
}

pub(super) async fn cache_file_metadata(
    pool: &SqlitePool,
    track_id: i64,
    metadata: Option<&str>,
) -> Result<()> {
    if let Some(metadata) = metadata {
        sqlx::query("UPDATE audio_files SET metadata_json=?, sample_rate=json_extract(?,'$.sample_rate'),total_frames=json_extract(?,'$.total_frames'),pcm_bits=json_extract(?,'$.pcm_bits'),pcm_float=coalesce(json_extract(?,'$.pcm_float'),0) WHERE id=(SELECT file_id FROM tracks WHERE id=?)")
            .bind(metadata).bind(metadata).bind(metadata).bind(metadata).bind(metadata).bind(track_id).execute(pool).await?;
    }
    Ok(())
}

pub(super) async fn upsert_track_by_path_norm(
    pool: &SqlitePool,
    input: UpsertTrackInput<'_>,
) -> Result<i64> {
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
    async fn multiple_artists_are_browsable_after_insert_and_rescan() {
        use crate::catalog::{CatalogQuery, CatalogSort, LocalCatalog, MediaKind};

        for normalized in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let pool = crate::worker::db::init_db(&dir.path().join("library.db"))
                .await
                .unwrap();
            let c = LocalCatalog::new(pool.clone());
            let mut q = CatalogQuery {
                source_instance_id: "1".into(),
                kind: MediaKind::Track,
                parent: None,
                search: String::new(),
                sort: CatalogSort::Default,
                cursor: None,
                limit: 20,
            };
            for (credit, expected) in [
                ("初音ミク / 鏡音リン", ["初音ミク", "鏡音リン"]),
                ("初音ミク、巡音ルカ", ["初音ミク", "巡音ルカ"]),
            ] {
                let tags = UpsertTrackInput {
                    artist: Some(credit),
                    album_artist: Some("初音ミク / Guest"),
                    album: Some("Album"),
                    ..input()
                };
                if normalized {
                    upsert_track_by_path_norm(&pool, tags).await.unwrap();
                } else {
                    upsert_track(&pool, tags).await.unwrap();
                }
                q.kind = MediaKind::Track;
                q.parent = None;
                let song = c.browse(&q).await.unwrap().items.remove(0);
                assert_eq!(song.artist_refs.len(), 2);
                let mut names = Vec::new();
                for reference in &song.artist_refs {
                    names.push(c.detail(reference).await.unwrap().title);
                    q.parent = Some(reference.clone());
                    assert_eq!(c.browse(&q).await.unwrap().items.len(), 1);
                    q.kind = MediaKind::Album;
                    assert_eq!(c.browse(&q).await.unwrap().items.len(), 1);
                    q.kind = MediaKind::Track;
                }
                assert_eq!(names, expected);
                q.kind = MediaKind::Artist;
                q.parent = None;
                let artists = c.browse(&q).await.unwrap().items;
                assert_eq!(artists.len(), 3); // Two performers plus Guest, with no duplicate lead.
                assert!(artists.iter().any(|a| a.title == "Guest"));
            }
            pool.close().await;
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
