use super::{CatalogItem, CatalogPage, CatalogQuery, CatalogSort, MediaKind, MediaRef};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

#[derive(Clone)]
pub struct LocalCatalog {
    pool: SqlitePool,
}

#[derive(Serialize, Deserialize)]
struct Cursor {
    query: String,
    revision: i64,
    offset: i64,
}

impl LocalCatalog {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    #[flutter_rust_bridge::frb(ignore)]
    pub async fn playback_resource(&self, track_id: i64) -> Result<super::LocalTrackResource> {
        let row = sqlx::query("SELECT t.path,t.start_frame,t.end_frame,t.segment_sample_rate,f.pcm_bits,f.pcm_float,coalesce(f.cover_key,t.id) AS cover_key FROM tracks t LEFT JOIN audio_files f ON f.id=t.file_id WHERE t.id=?")
            .bind(track_id).fetch_optional(&self.pool).await?.ok_or_else(|| anyhow!("local track not found: {track_id}"))?;
        let start: Option<i64> = row.try_get("start_frame")?;
        Ok(super::LocalTrackResource {
            path: row.try_get("path")?,
            segment: start
                .map(|start| {
                    Ok::<_, sqlx::Error>(stellatune_audio_core::segment::AudioSegment {
                        start_frame: start as u64,
                        end_frame_exclusive: row.try_get::<i64, _>("end_frame")? as u64,
                        sample_rate: row.try_get::<i64, _>("segment_sample_rate")? as u32,
                    })
                })
                .transpose()?,
            cover_key: row.try_get("cover_key")?,
            pcm_bits: row.try_get::<Option<i64>, _>("pcm_bits")?.map(|v| v as u32),
            pcm_float: row
                .try_get::<Option<bool>, _>("pcm_float")?
                .unwrap_or(false),
        })
    }

    pub async fn browse(&self, query: &CatalogQuery) -> Result<CatalogPage> {
        self.query(query, None).await
    }

    pub async fn detail(&self, reference: &MediaRef) -> Result<CatalogItem> {
        if reference.kind == MediaKind::Folder {
            let found: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM tracks WHERE dir_norm=? OR substr(dir_norm,1,length(?))=?",
            )
            .bind(&reference.id)
            .bind(format!("{}/", reference.id.trim_end_matches('/')))
            .bind(format!("{}/", reference.id.trim_end_matches('/')))
            .fetch_one(&self.pool)
            .await?;
            let roots: Vec<String> = sqlx::query_scalar("SELECT path FROM scan_roots")
                .fetch_all(&self.pool)
                .await?;
            if found == 0
                && !roots
                    .iter()
                    .any(|p| crate::worker::paths::normalize_path_str(p) == reference.id)
            {
                bail!("catalog item not found");
            }
            return Ok(empty_item(
                reference.clone(),
                reference
                    .id
                    .rsplit('/')
                    .find(|s| !s.is_empty())
                    .unwrap_or("/")
                    .into(),
            ));
        }
        let q = CatalogQuery {
            source_instance_id: reference.source_instance_id.clone(),
            kind: reference.kind,
            parent: None,
            search: String::new(),
            sort: CatalogSort::Default,
            cursor: None,
            limit: 1,
        };
        self.query(&q, Some(&reference.id))
            .await?
            .items
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("catalog item not found"))
    }

    async fn query(&self, q: &CatalogQuery, exact_id: Option<&str>) -> Result<CatalogPage> {
        q.validate()?;
        let mut identity = q.clone();
        identity.cursor = None;
        let fingerprint = serde_json::to_string(&identity)?;
        let mut tx = self.pool.begin().await?;
        let revision: i64 = sqlx::query_scalar("SELECT revision FROM catalog_revision")
            .fetch_one(&mut *tx)
            .await?;
        let offset = match &q.cursor {
            None => 0,
            Some(raw) => {
                let c: Cursor = serde_json::from_str(raw)?;
                if c.query != fingerprint || c.revision != revision || c.offset < 0 {
                    bail!("catalog cursor expired; refresh the collection");
                }
                c.offset
            },
        };
        let mut sql = QueryBuilder::<Sqlite>::new("SELECT * FROM (");
        match q.kind {
            MediaKind::Track => {
                // Keep the filename fallback in SQL so title sorting/search use
                // the same label as the UI. Paths may mix Windows separators.
                // Only strip the recorded extension from the filename fallback;
                // dots in real title tags and elsewhere in filenames are retained.
                sql.push("SELECT CAST(t.id AS TEXT) AS entity_id, coalesce(nullif(trim(t.title),''),CASE WHEN t.ext<>'' AND length(t.filename)>length(t.ext)+1 AND lower(substr(t.filename,-length(t.ext)-1))='.'||lower(t.ext) THEN substr(t.filename,1,length(t.filename)-length(t.ext)-1) ELSE t.filename END) AS name, t.*, NULL AS count FROM (SELECT catalog_tracks.*,ltrim(substr(replace(path,char(92),'/'),length(dir_norm)+1),'/') AS filename FROM catalog_tracks) t WHERE 1=1");
                if let Some(parent) = &q.parent {
                    match parent.kind {
                        MediaKind::Album => {
                            sql.push(" AND t.album_key=").push_bind(&parent.id);
                        },
                        MediaKind::Artist => {
                            sql.push(
                                " AND EXISTS(SELECT 1 FROM catalog_artists WHERE track_id=t.id AND name=",
                            )
                            .push_bind(serde_json::from_str::<String>(&parent.id)?)
                            .push(")");
                        },
                        MediaKind::Folder => {
                            sql.push(" AND t.dir_norm=").push_bind(&parent.id);
                        },
                        MediaKind::Playlist => {
                            sql.push(" AND EXISTS(SELECT 1 FROM playlist_tracks p WHERE p.track_id=t.id AND CAST(p.playlist_id AS TEXT)=").push_bind(&parent.id).push(")");
                        },
                        _ => bail!("tracks cannot contain other tracks"),
                    }
                }
            },
            MediaKind::Album => {
                sql.push("SELECT album_key AS entity_id, trim(coalesce(album,'')) AS name, trim(coalesce(nullif(trim(album_artist),''),artist,'')) AS artist, count(*) AS count, min(id) AS cover_id FROM catalog_tracks t WHERE 1=1");
                if let Some(parent) = &q.parent {
                    if parent.kind != MediaKind::Artist {
                        bail!("albums can only be browsed under artists");
                    }
                    sql.push(" AND album_key IN (SELECT a.album_key FROM catalog_tracks a JOIN catalog_artists ca ON ca.track_id=a.id WHERE ca.name=").push_bind(serde_json::from_str::<String>(&parent.id)?).push(")");
                }
                sql.push(" GROUP BY album_key");
            },
            MediaKind::Artist => {
                if q.parent.is_some() {
                    bail!("artist list has no parent");
                }
                sql.push("SELECT json_quote(name) AS entity_id,name,count(*) AS count,min(track_id) AS cover_id FROM catalog_artists GROUP BY name");
            },
            MediaKind::Playlist => {
                if q.parent.is_some() {
                    bail!("playlist list has no parent");
                }
                sql.push("SELECT CAST(p.id AS TEXT) AS entity_id,p.name,count(pt.track_id) AS count,min(pt.track_id) AS cover_id FROM playlists p LEFT JOIN playlist_tracks pt ON pt.playlist_id=p.id GROUP BY p.id");
            },
            MediaKind::Folder => {
                if let Some(parent) = &q.parent {
                    if parent.kind != MediaKind::Folder {
                        bail!("folder parent must be a folder");
                    }
                    let prefix = format!("{}/", parent.id.trim_end_matches('/'));
                    sql.push("SELECT DISTINCT ").push_bind(prefix.clone()).push(" || child AS entity_id, child AS name FROM (SELECT CASE WHEN instr(rest,'/')=0 THEN rest ELSE substr(rest,1,instr(rest,'/')-1) END AS child FROM (SELECT substr(dir_norm,")
                        .push_bind(prefix.chars().count() as i64 + 1).push(") AS rest FROM tracks WHERE substr(dir_norm,1,")
                        .push_bind(prefix.chars().count() as i64).push(")=").push_bind(prefix).push(")) WHERE child<>''");
                } else {
                    let roots: Vec<String> = sqlx::query_scalar(
                        "SELECT path FROM scan_roots WHERE enabled=1 ORDER BY path",
                    )
                    .fetch_all(&mut *tx)
                    .await?;
                    let roots: Vec<String> = roots
                        .iter()
                        .map(|s| crate::worker::paths::normalize_path_str(s))
                        .collect();
                    sql.push("SELECT value AS entity_id, value AS name FROM json_each(")
                        .push_bind(serde_json::to_string(&roots)?)
                        .push(")");
                }
            },
        }
        sql.push(") WHERE 1=1");
        if let Some(id) = exact_id {
            sql.push(" AND entity_id=").push_bind(id);
        }
        if !q.search.trim().is_empty() {
            let pattern = format!(
                "%{}%",
                q.search
                    .trim()
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            );
            sql.push(" AND (name LIKE ")
                .push_bind(pattern.clone())
                .push(" ESCAPE '\\'");
            if q.kind == MediaKind::Track {
                sql.push(" OR artist LIKE ")
                    .push_bind(pattern.clone())
                    .push(" ESCAPE '\\' OR album LIKE ")
                    .push_bind(pattern)
                    .push(" ESCAPE '\\'");
            }
            sql.push(")");
        }
        if q.sort == CatalogSort::Default
            && q.kind == MediaKind::Track
            && q.parent
                .as_ref()
                .is_some_and(|p| p.kind == MediaKind::Album)
        {
            sql.push(" ORDER BY disc_number IS NULL,disc_number,track_number IS NULL,track_number,name COLLATE NOCASE,entity_id");
        } else if q.sort == CatalogSort::Default
            && q.kind == MediaKind::Track
            && q.parent
                .as_ref()
                .is_some_and(|p| p.kind == MediaKind::Playlist)
        {
            sql.push(" ORDER BY (SELECT sort_index FROM playlist_tracks pt WHERE pt.track_id=id AND CAST(pt.playlist_id AS TEXT)=").push_bind(&q.parent.as_ref().unwrap().id).push("),entity_id");
        } else if q.sort == CatalogSort::Default && q.kind == MediaKind::Track {
            // Stable numeric tie-breaker keeps equal timestamps consistent across pages.
            sql.push(" ORDER BY mtime_ms DESC,id DESC");
        } else {
            sql.push(" ORDER BY name COLLATE NOCASE,entity_id");
        }
        sql.push(" LIMIT ")
            .push_bind(i64::from(q.limit) + 1)
            .push(" OFFSET ")
            .push_bind(offset);
        let rows = sql.build().fetch_all(&mut *tx).await?;
        let more = rows.len() > q.limit as usize;
        let mut items = Vec::new();
        for row in rows.into_iter().take(q.limit as usize) {
            let reference = MediaRef {
                source_instance_id: q.source_instance_id.clone(),
                kind: q.kind,
                id: row.try_get("entity_id")?,
            };
            let mut item = empty_item(reference, row.try_get("name")?);
            item.artist = row.try_get("artist").unwrap_or(None);
            item.track_count = row.try_get("count").unwrap_or(None);
            if q.kind == MediaKind::Track {
                item.local_track_id = Some(row.try_get("id")?);
                item.local_path = Some(row.try_get("path")?);
                item.album = row.try_get("album")?;
                item.duration_ms = row.try_get("duration_ms")?;
                item.audio = Some(super::CatalogAudioInfo {
                    format: row.try_get::<String, _>("ext")?.to_uppercase(),
                    codec: None,
                    sample_rate: None,
                    bits_per_sample: None,
                    floating_point: false,
                    channels: None,
                    bitrate: None,
                    cue_path: row.try_get("cue_path")?,
                    start_frame: row.try_get("start_frame")?,
                    end_frame: row.try_get("end_frame")?,
                    disc_number: row.try_get("disc_number")?,
                    track_number: row.try_get("track_number")?,
                    source_directory: row.try_get("dir_norm")?,
                });
                item.album_ref = Some(MediaRef {
                    source_instance_id: q.source_instance_id.clone(),
                    kind: MediaKind::Album,
                    id: row.try_get("album_key")?,
                });
                let artists: Vec<String> =
                    serde_json::from_str(&row.try_get::<String, _>("artist_names")?)?;
                item.artist_refs = artists
                    .into_iter()
                    .map(|id| MediaRef {
                        source_instance_id: q.source_instance_id.clone(),
                        kind: MediaKind::Artist,
                        id: serde_json::to_string(&id).expect("string JSON"),
                    })
                    .collect();
            } else {
                // A local cover ID is presentation only, never this entity's identity.
                item.local_track_id = row.try_get("cover_id").unwrap_or(None);
            }
            items.push(item);
        }
        tx.commit().await?;
        let attributes = crate::worker::tracks::attributes(
            &self.pool,
            &items
                .iter()
                .filter_map(|i| i.local_track_id)
                .collect::<Vec<_>>(),
        )
        .await?;
        for item in &mut items {
            if let Some((cover, segment)) = item.local_track_id.and_then(|id| attributes.get(&id)) {
                item.local_cover_id = Some(*cover);
                item.is_segment = item.reference.kind == MediaKind::Track && *segment;
            }
        }
        // One bounded database query per page, including files shared by CUE tracks.
        let ids: Vec<_> = items
            .iter()
            .filter(|i| i.audio.is_some())
            .filter_map(|i| i.local_track_id)
            .collect();
        if !ids.is_empty() {
            let mut props = QueryBuilder::<Sqlite>::new(
                "SELECT t.id,f.sample_rate,f.pcm_bits,f.pcm_float,f.metadata_json,CASE WHEN f.probe_mtime_ms=f.mtime_ms AND f.probe_size_bytes=f.size_bytes THEN f.properties_json END AS properties_json FROM tracks t LEFT JOIN audio_files f ON f.id=t.file_id WHERE t.id IN (",
            );
            let mut list = props.separated(",");
            for id in ids {
                list.push_bind(id);
            }
            list.push_unseparated(")");
            let rows = props.build().fetch_all(&self.pool).await?;
            for row in rows {
                let id: i64 = row.try_get("id")?;
                let Some(audio) = items
                    .iter_mut()
                    .find(|i| i.local_track_id == Some(id))
                    .and_then(|i| i.audio.as_mut())
                else {
                    continue;
                };
                audio.sample_rate = row
                    .try_get::<Option<i64>, _>("sample_rate")?
                    .map(|v| v as u32);
                audio.bits_per_sample =
                    row.try_get::<Option<i64>, _>("pcm_bits")?.map(|v| v as u32);
                audio.floating_point = row
                    .try_get::<Option<bool>, _>("pcm_float")?
                    .unwrap_or(false);
                let meta: serde_json::Value = row
                    .try_get::<Option<String>, _>("metadata_json")?
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();
                audio.channels = meta["channels"].as_u64().and_then(|n| n.try_into().ok());
                audio.codec = meta["codec"].as_str().map(str::to_owned);
                if let Some(p) = row
                    .try_get::<Option<String>, _>("properties_json")?
                    .and_then(|s| {
                        serde_json::from_str::<stellatune_media_probe::AudioProperties>(&s).ok()
                    })
                {
                    audio.sample_rate = audio.sample_rate.or(p.sample_rate);
                    audio.bits_per_sample = p.bits_per_sample.or(audio.bits_per_sample);
                    audio.floating_point |= p.floating_point;
                    audio.channels = audio.channels.or(p.channels);
                    audio.codec = audio.codec.take().or(p.codec);
                    audio.bitrate = p.bitrate;
                }
            }
        }
        Ok(CatalogPage {
            next_cursor: if more {
                Some(serde_json::to_string(&Cursor {
                    query: fingerprint,
                    revision,
                    offset: offset + i64::from(q.limit),
                })?)
            } else {
                None
            },
            items,
            total: None,
        })
    }
}

fn empty_item(reference: MediaRef, title: String) -> CatalogItem {
    CatalogItem {
        reference,
        title,
        artist: None,
        album: None,
        duration_ms: None,
        track_count: None,
        artwork_url: None,
        local_track_id: None,
        local_cover_id: None,
        is_segment: false,
        audio: None,
        local_path: None,
        album_ref: None,
        artist_refs: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn catalog() -> (tempfile::TempDir, LocalCatalog) {
        let dir = tempfile::tempdir().unwrap();
        let pool = crate::worker::db::init_db(&dir.path().join("library.db"))
            .await
            .unwrap();
        (dir, LocalCatalog::new(pool))
    }
    #[tokio::test]
    async fn browsing_returns_cached_audio_properties_without_opening_source_files() {
        let (_dir, c) = catalog().await;
        track(&c, 1, "Album", "Artist", "/missing").await;
        let file: i64 = sqlx::query_scalar("INSERT INTO audio_files(path,path_norm,ext,mtime_ms,size_bytes,sample_rate,pcm_bits,pcm_float,metadata_json) VALUES('/missing/1.flac','/missing/1.flac','flac',0,0,96000,24,0,'{\"codec\":\"flac\",\"channels\":2}') RETURNING id")
            .fetch_one(&c.pool).await.unwrap();
        sqlx::query("UPDATE tracks SET file_id=?,ext='flac' WHERE id=1")
            .bind(file)
            .execute(&c.pool)
            .await
            .unwrap();
        let items = c.browse(&query(MediaKind::Track)).await.unwrap().items;
        let info = items[0].audio.as_ref().unwrap();
        assert_eq!(info.format, "FLAC");
        assert_eq!(info.codec.as_deref(), Some("flac"));
        assert_eq!(info.sample_rate, Some(96000));
        assert_eq!(info.bits_per_sample, Some(24));
        assert_eq!(info.channels, Some(2));
        assert_eq!(info.source_directory, "/missing");
        assert!(info.bitrate.is_none());
        sqlx::query("UPDATE audio_files SET probe_mtime_ms=mtime_ms,probe_size_bytes=size_bytes,properties_json='{\"bitrate\":{\"bps\":128000,\"kind\":\"average\",\"estimated\":true,\"mode\":null}}' WHERE id=?")
            .bind(file).execute(&c.pool).await.unwrap();
        let item = c.detail(&items[0].reference).await.unwrap();
        let bitrate = item.audio.as_ref().unwrap().bitrate.as_ref().unwrap();
        assert_eq!(bitrate.bps, 128000);
        assert!(bitrate.estimated);
        assert!(bitrate.mode.is_none());
    }
    fn query(kind: MediaKind) -> CatalogQuery {
        CatalogQuery {
            source_instance_id: "1".into(),
            kind,
            parent: None,
            search: String::new(),
            sort: CatalogSort::Default,
            cursor: None,
            limit: 2,
        }
    }
    async fn track(c: &LocalCatalog, n: i64, album: &str, artist: &str, dir: &str) {
        sqlx::query("INSERT INTO tracks(id,path,path_norm,dir_norm,mtime_ms,size_bytes,title,album,artist,album_artist,disc_number,track_number) VALUES(?,?,?,?,0,0,?,?,?,'Various',1,?)")
            .bind(n).bind(format!("{dir}/{n}.flac")).bind(format!("{dir}/{n}.flac")).bind(dir)
            .bind(format!("Song {n}")).bind(album).bind(artist).bind(10-n).execute(&c.pool).await.unwrap();
    }
    #[tokio::test]
    async fn missing_track_titles_use_filenames_for_display_sort_and_search() {
        let (_dir, c) = catalog().await;
        for (id, path, dir, title) in [
            (1, "D:/音乐/Z\\Alpha.mp3", "D:/音乐/Z", None),
            (2, "/music/A/Beta.flac", "/music/A", Some("")),
            (3, "D:\\Music\\歌曲.mp3", "D:/Music", Some("   ")),
            (4, "/music/tagged.flac", "/music", Some("AC/DC / Live")),
            (5, "/music/Vol.2.Live.MP3", "/music", None),
            (6, "/music/tagged-title.mp3", "/music", Some("Title.mp3")),
            (7, "/music/No extension", "/music", None),
        ] {
            track(&c, id, "Album", "A", dir).await;
            let normalized = crate::worker::paths::normalize_path_str(path);
            let extension = std::path::Path::new(&normalized)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_lowercase();
            sqlx::query("UPDATE tracks SET path=?,path_norm=?,title=?,ext=? WHERE id=?")
                .bind(path)
                .bind(normalized)
                .bind(title)
                .bind(extension)
                .bind(id)
                .execute(&c.pool)
                .await
                .unwrap();
        }
        let mut q = query(MediaKind::Track);
        q.sort = CatalogSort::Title;
        q.limit = 20;
        let page = c.browse(&q).await.unwrap();
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            [
                "AC/DC / Live",
                "Alpha",
                "Beta",
                "No extension",
                "Title.mp3",
                "Vol.2.Live",
                "歌曲"
            ]
        );
        let alpha = &page.items[1];
        assert_eq!(alpha.local_path.as_deref(), Some("D:/音乐/Z\\Alpha.mp3"));
        assert_eq!(c.detail(&alpha.reference).await.unwrap().title, "Alpha");
        q.search = "Alpha".into();
        assert_eq!(c.browse(&q).await.unwrap().items.len(), 1);
        q.search = "D:/音乐".into();
        assert!(c.browse(&q).await.unwrap().items.is_empty());
    }

    #[tokio::test]
    async fn default_tracks_use_modification_time_across_pages_and_folders() {
        let (_dir, c) = catalog().await;
        for (id, modified) in [(1, 100), (2, 300), (3, 200), (10, 300), (5, 0)] {
            track(
                &c,
                id,
                "Album",
                "A",
                if id == 3 { "/other" } else { "/music" },
            )
            .await;
            sqlx::query("UPDATE tracks SET mtime_ms=? WHERE id=?")
                .bind(modified)
                .bind(id)
                .execute(&c.pool)
                .await
                .unwrap();
        }
        let mut q = query(MediaKind::Track);
        let mut ids = Vec::new();
        loop {
            let page = c.browse(&q).await.unwrap();
            ids.extend(page.items.iter().map(|item| item.local_track_id.unwrap()));
            q.cursor = page.next_cursor;
            if q.cursor.is_none() {
                break;
            }
        }
        assert_eq!(ids, [10, 2, 3, 1, 5]);
        q.limit = 20;
        q.parent = Some(MediaRef {
            source_instance_id: "1".into(),
            kind: MediaKind::Folder,
            id: "/music".into(),
        });
        let page = c.browse(&q).await.unwrap();
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.local_track_id.unwrap())
                .collect::<Vec<_>>(),
            [10, 2, 1, 5]
        );
        q.parent = None;
        q.sort = CatalogSort::Title;
        let page = c.browse(&q).await.unwrap();
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.local_track_id.unwrap())
                .collect::<Vec<_>>(),
            [1, 10, 2, 3, 5]
        );
    }

    #[tokio::test]
    async fn complete_albums_pagination_compilations_and_order() {
        let (_dir, c) = catalog().await;
        for n in 1..=5 {
            track(
                &c,
                n,
                if n < 3 { "Compilation" } else { "Other" },
                if n == 1 { "A" } else { "B" },
                "/music",
            )
            .await;
        }
        track(&c, 6, "Third", "C", "/music").await;
        let page = c.browse(&query(MediaKind::Album)).await.unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(page.next_cursor.is_some());
        assert_eq!(page.items[0].track_count, Some(2));
        let mut next = query(MediaKind::Album);
        next.cursor = page.next_cursor;
        assert_eq!(c.browse(&next).await.unwrap().items[0].title, "Third");
        let mut tracks = query(MediaKind::Track);
        tracks.parent = Some(page.items[0].reference.clone());
        assert_eq!(
            c.browse(&tracks)
                .await
                .unwrap()
                .items
                .iter()
                .map(|t| t.local_track_id.unwrap())
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
        let mut albums = query(MediaKind::Album);
        albums.parent = Some(MediaRef {
            source_instance_id: "1".into(),
            kind: MediaKind::Artist,
            id: "\"A\"".into(),
        });
        let page = c.browse(&albums).await.unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].track_count, Some(2));
    }
    #[tokio::test]
    async fn cursor_is_bound_to_query_and_scan_revision() {
        let (_dir, c) = catalog().await;
        for n in 1..=4 {
            track(&c, n, "Album", "A", "/music").await;
        }
        let mut q = query(MediaKind::Track);
        q.cursor = c.browse(&q).await.unwrap().next_cursor;
        let mut other = q.clone();
        other.search = "other".into();
        assert!(c.browse(&other).await.is_err());
        sqlx::query("DELETE FROM tracks WHERE id=4")
            .execute(&c.pool)
            .await
            .unwrap();
        assert!(
            c.browse(&q)
                .await
                .unwrap_err()
                .to_string()
                .contains("expired")
        );
    }
    #[tokio::test]
    async fn unknown_albums_unicode_directories_and_unknown_artists() {
        let (_dir, c) = catalog().await;
        track(&c, 1, "", "", "/音乐/一").await;
        track(&c, 2, "", "", "/音乐/二").await;
        let mut q = query(MediaKind::Folder);
        q.parent = Some(MediaRef {
            source_instance_id: "1".into(),
            kind: MediaKind::Folder,
            id: "/音乐".into(),
        });
        let page = c.browse(&q).await.unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].reference.id, "/音乐/一");
        assert_eq!(
            c.browse(&query(MediaKind::Album))
                .await
                .unwrap()
                .items
                .len(),
            2
        );
        let mut artists = query(MediaKind::Artist);
        artists.limit = 20;
        let page = c.browse(&artists).await.unwrap();
        let unknown = page.items.iter().find(|v| v.title.is_empty()).unwrap();
        let mut tracks = query(MediaKind::Track);
        tracks.parent = Some(unknown.reference.clone());
        assert_eq!(c.browse(&tracks).await.unwrap().items.len(), 2);
    }
    #[tokio::test]
    async fn playlist_order_and_detail_are_database_queries() {
        let (_dir, c) = catalog().await;
        for n in 1..=3 {
            track(&c, n, "Album", "A", "/music").await;
        }
        let id: i64 = sqlx::query_scalar("INSERT INTO playlists(name) VALUES('Test') RETURNING id")
            .fetch_one(&c.pool)
            .await
            .unwrap();
        for (n, order) in [(1, 2), (2, 0), (3, 1)] {
            sqlx::query(
                "INSERT INTO playlist_tracks(playlist_id,track_id,sort_index) VALUES(?,?,?)",
            )
            .bind(id)
            .bind(n)
            .bind(order)
            .execute(&c.pool)
            .await
            .unwrap();
        }
        let reference = MediaRef {
            source_instance_id: "1".into(),
            kind: MediaKind::Playlist,
            id: id.to_string(),
        };
        assert_eq!(c.detail(&reference).await.unwrap().track_count, Some(3));
        let mut q = query(MediaKind::Track);
        q.parent = Some(reference);
        assert_eq!(
            c.browse(&q)
                .await
                .unwrap()
                .items
                .iter()
                .map(|i| i.local_track_id.unwrap())
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }
}
