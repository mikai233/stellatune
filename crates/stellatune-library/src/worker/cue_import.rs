use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use sqlx::{Row, SqlitePool};

use super::metadata::{ExtractedMetadata, extract_metadata, write_cover_bytes};
use super::paths::{is_under_excluded, normalize_path_str, now_ms, parent_dir_norm};

use crate::cue::CueSheet;
use crate::service::EventHub;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Document {
    sheet: CueSheet,
    files: BTreeMap<String, i64>,
}

fn normalized(path: &Path) -> Result<String> {
    Ok(normalize_path_str(&path.canonicalize()?.to_string_lossy())
        .trim_start_matches("//?/")
        .to_owned())
}

fn beneath(path: &str, roots: &[String]) -> bool {
    if cfg!(windows) {
        is_under_excluded(
            &path.to_lowercase(),
            &roots.iter().map(|r| r.to_lowercase()).collect::<Vec<_>>(),
        )
    } else {
        is_under_excluded(path, roots)
    }
}

async fn file(
    pool: &SqlitePool,
    path: &Path,
    covers: &Path,
    force: bool,
    retry_io: bool,
) -> Result<(i64, bool)> {
    let norm = normalized(path)?;
    let stat = std::fs::metadata(path)?;
    let mtime = stat
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let old = sqlx::query(
        "SELECT id,mtime_ms,size_bytes,total_frames FROM audio_files WHERE path_norm=?",
    )
    .bind(&norm)
    .fetch_optional(pool)
    .await?;
    if !force
        && let Some(old) = old.as_ref()
        && old.get::<i64, _>("mtime_ms") == mtime
        && old.get::<i64, _>("size_bytes") == stat.len() as i64
        && old.get::<Option<i64>, _>("total_frames").is_some()
    {
        let changed = super::properties::refresh(pool, path, &None, false, retry_io).await?;
        return Ok((old.get("id"), changed));
    }
    let path_owned = path.to_owned();
    let metadata = tokio::task::spawn_blocking(move || extract_metadata(&path_owned)).await??;
    let sample_rate = metadata
        .sample_rate
        .context("CUE requires a known sample rate")?;
    let total = metadata
        .total_frames
        .context("CUE requires an exact sample count")?;
    if total == 0 {
        bail!("empty CUE audio source");
    }
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    anyhow::ensure!(
        super::properties::matches(path, mtime, stat.len() as i64),
        "CUE audio changed while scanning"
    );
    let id: i64 = sqlx::query_scalar("INSERT INTO audio_files(path,path_norm,ext,mtime_ms,size_bytes,meta_scanned_ms,sample_rate,total_frames,pcm_bits,pcm_float,metadata_json) VALUES(?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(path_norm) DO UPDATE SET path=excluded.path,mtime_ms=excluded.mtime_ms,size_bytes=excluded.size_bytes,meta_scanned_ms=excluded.meta_scanned_ms,sample_rate=excluded.sample_rate,total_frames=excluded.total_frames,pcm_bits=excluded.pcm_bits,pcm_float=excluded.pcm_float,metadata_json=excluded.metadata_json RETURNING id")
        .bind(&norm).bind(&norm).bind(ext).bind(mtime).bind(stat.len() as i64).bind(now_ms()).bind(sample_rate).bind(i64::try_from(total)?).bind(metadata.pcm_bits).bind(metadata.pcm_float).bind(serde_json::to_string(&metadata)?).fetch_one(pool).await?;
    let changed = super::properties::refresh(pool, path, &None, force, retry_io).await?;
    sqlx::query("UPDATE audio_files SET cover_key=? WHERE id=?")
        .bind(-id)
        .bind(id)
        .execute(pool)
        .await?;
    if let Some(cover) = metadata.cover {
        write_cover_bytes(covers, -id, &cover)?;
    }
    Ok((id, changed))
}

/// Reconcile a complete set of CUE directories. Unchanged documents do not write
/// tracks, while conflicts include other already imported documents.
pub(super) async fn reconcile(
    pool: &SqlitePool,
    events: &Arc<EventHub>,
    covers: &Path,
    directories: &[String],
    recursive: bool,
    force: bool,
    retry_io: bool,
) -> Result<bool> {
    let roots: Vec<String> = sqlx::query_scalar("SELECT path FROM scan_roots WHERE enabled=1")
        .fetch_all(pool)
        .await?;
    let roots = roots
        .iter()
        .filter_map(|r| normalized(Path::new(r)).ok())
        .collect::<Vec<_>>();
    let excluded: Vec<String> = sqlx::query_scalar("SELECT path FROM excluded_folders")
        .fetch_all(pool)
        .await?;
    let excluded = excluded
        .iter()
        .map(|p| normalized(Path::new(p)).unwrap_or_else(|_| normalize_path_str(p)))
        .collect::<Vec<_>>();
    let old = sqlx::query("SELECT path,fingerprint,signature,document_json FROM cue_documents")
        .fetch_all(pool)
        .await?;
    let mut documents = BTreeMap::<String, (String, String, Document)>::new();
    for row in &old {
        documents.insert(
            row.get("path"),
            (
                row.get("fingerprint"),
                row.get("signature"),
                serde_json::from_str(row.get("document_json"))?,
            ),
        );
    }
    let directories = directories
        .iter()
        .map(|p| normalized(Path::new(p)).unwrap_or_else(|_| normalize_path_str(p)))
        .collect::<Vec<_>>();
    let affected = |path: &str| {
        directories.iter().any(|dir| {
            if recursive {
                beneath(path, std::slice::from_ref(dir))
            } else {
                parent_dir_norm(path).as_ref() == Some(dir)
            }
        })
    };
    let mut paths = BTreeSet::new();
    for dir in &directories {
        for entry in walkdir::WalkDir::new(dir)
            .follow_links(false)
            .max_depth(if recursive { usize::MAX } else { 1 })
        {
            let Ok(entry) = entry else {
                continue;
            };
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|v| v.to_str())
                    .is_some_and(|v| v.eq_ignore_ascii_case("cue"))
            {
                paths.insert(normalized(entry.path())?);
            }
        }
    }
    let mut removed = HashSet::new();
    for path in documents.keys() {
        if !beneath(path, &roots)
            || beneath(path, &excluded)
            || (affected(path)
                && std::fs::metadata(path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound))
        {
            removed.insert(path.clone());
        }
    }
    for path in &removed {
        documents.remove(path);
    }
    let mut properties_changed = false;
    let mut probed_files = HashMap::<String, i64>::new();
    for path in paths {
        if !beneath(&path, &roots) || beneath(&path, &excluded) {
            continue;
        }
        let result: Result<_> = async {
            let task_path = PathBuf::from(&path);
            let mut sheet = tokio::task::spawn_blocking(move || crate::cue::read(&task_path)).await??;
            let mut files = BTreeMap::new();
            let mut fingerprints = Vec::new();
            for track in &mut sheet.tracks {
                let resolved = crate::cue::resolve_file(Path::new(&path).parent().context("CUE parent")?, &track.file)?;
                let norm = normalized(&resolved)?;
                if !beneath(&norm, &roots) || beneath(&norm, &excluded) { bail!("CUE references audio outside enabled scan roots or inside an excluded folder"); }
                let id = if let Some(id) = files.get(&norm) { *id } else {
                    let id = if let Some(id) = probed_files.get(&norm) { *id } else { let (id, updated) = file(pool, &resolved, covers, force, retry_io).await?; properties_changed |= updated; probed_files.insert(norm.clone(),id); id };
                    files.insert(norm.clone(), id);
                    let row = sqlx::query("SELECT mtime_ms,size_bytes FROM audio_files WHERE id=?").bind(id).fetch_one(pool).await?;
                    fingerprints.push((norm.clone(), row.get::<i64,_>("mtime_ms"), row.get::<i64,_>("size_bytes")));
                    id
                };
                let row = sqlx::query("SELECT sample_rate,total_frames FROM audio_files WHERE id=?").bind(id).fetch_one(pool).await?;
                let start = crate::cue::sample_frame(track.index, row.get::<i64,_>("sample_rate") as u32)?;
                if start >= row.get::<i64,_>("total_frames") as u64 { bail!("CUE index exceeds source length"); }
                track.file = norm;
            }
            let signature = serde_json::to_string(&sheet)?;
            let fingerprint = serde_json::to_string(&fingerprints)?;
            Ok((fingerprint, signature, Document { sheet, files }))
        }.await;
        match result {
            Ok(document) => {
                documents.insert(path, document);
            },
            Err(error) => events.emit(crate::LibraryEvent::Log {
                message: format!("CUE import failed: {path}: {error:#}"),
            }),
        }
    }
    // A file may be referenced by duplicate cue sheets, but never by conflicting
    // descriptions. Decide the whole batch before changing any visible rows.
    let mut claims = HashMap::<String, Vec<(String, String)>>::new();
    for (path, (_, signature, document)) in &documents {
        for file in document.files.keys() {
            claims
                .entry(file.clone())
                .or_default()
                .push((path.clone(), signature.clone()));
        }
    }
    let mut blocked = HashSet::new();
    for (file, entries) in claims {
        if entries
            .iter()
            .any(|(_, signature)| signature != &entries[0].1)
        {
            events.emit(crate::LibraryEvent::Log {
                message: format!(
                    "Conflicting CUE sheets reference {file}; retaining whole-file entry"
                ),
            });
            blocked.extend(entries.into_iter().map(|(path, _)| path));
        }
    }
    let mut tx = pool.begin().await?;
    let mut changed = properties_changed;
    let mut restore = HashSet::new();
    for path in &removed {
        let ids: Vec<i64> =
            sqlx::query_scalar("SELECT DISTINCT file_id FROM tracks WHERE cue_path=?")
                .bind(path)
                .fetch_all(&mut *tx)
                .await?;
        restore.extend(ids);
        sqlx::query("DELETE FROM cue_documents WHERE path=?")
            .bind(path)
            .execute(&mut *tx)
            .await?;
        changed = true;
    }
    let mut signatures = HashSet::new();
    for (path, (fingerprint, signature, document)) in documents {
        let inactive = blocked.contains(&path) || !signatures.insert(signature.clone());
        let previous = old.iter().find(|row| row.get::<&str, _>("path") == path);
        let existing: i64 = sqlx::query_scalar("SELECT count(*) FROM tracks WHERE cue_path=?")
            .bind(&path)
            .fetch_one(&mut *tx)
            .await?;
        if !force
            && previous.is_some_and(|row| {
                row.get::<&str, _>("fingerprint") == fingerprint
                    && row.get::<&str, _>("signature") == signature
            })
            && ((inactive && existing == 0)
                || (!inactive && existing == document.sheet.tracks.len() as i64))
        {
            continue;
        }
        if let Some(previous) = previous {
            let old_document: Document = serde_json::from_str(previous.get("document_json"))?;
            restore.extend(old_document.files.values().copied());
        }
        restore.extend(document.files.values().copied());
        sqlx::query("INSERT INTO cue_documents(path,fingerprint,signature,document_json) VALUES(?,?,?,?) ON CONFLICT(path) DO UPDATE SET fingerprint=excluded.fingerprint,signature=excluded.signature,document_json=excluded.document_json")
            .bind(&path).bind(fingerprint).bind(signature).bind(serde_json::to_string(&document)?).execute(&mut *tx).await?;
        if inactive {
            sqlx::query("DELETE FROM tracks WHERE cue_path=?")
                .bind(&path)
                .execute(&mut *tx)
                .await?;
            changed = true;
            continue;
        }
        let mut keys = HashSet::new();
        for (index, track) in document.sheet.tracks.iter().enumerate() {
            let id = document.files[&track.file];
            let row = sqlx::query("SELECT * FROM audio_files WHERE id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
            let metadata: ExtractedMetadata = serde_json::from_str(row.get("metadata_json"))?;
            let rate = row.get::<i64, _>("sample_rate") as u32;
            let start = crate::cue::sample_frame(track.index, rate)?;
            let end_cd = document
                .sheet
                .tracks
                .iter()
                .skip(index + 1)
                .find(|t| t.file == track.file)
                .map(|t| t.index);
            let end = end_cd
                .map(|v| crate::cue::sample_frame(v, rate))
                .transpose()?
                .unwrap_or(row.get::<i64, _>("total_frames") as u64);
            if end <= start {
                bail!("CUE contains an empty or backwards segment");
            }
            let title = track
                .title
                .as_deref()
                .or(document.sheet.title.as_deref())
                .or(metadata.title.as_deref())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Track {:02}", track.number));
            let artist = track
                .performer
                .as_deref()
                .or(document.sheet.performer.as_deref())
                .or(metadata.artist.as_deref());
            let album_artist = document
                .sheet
                .performer
                .as_deref()
                .or(metadata.album_artist.as_deref())
                .or(metadata.artist.as_deref());
            let artists = crate::artist_names::normalize_artists_json("[]", artist)?;
            let album_artists = crate::artist_names::normalize_artists_json("[]", album_artist)?;
            let key = format!("cue:{path}:{}", track.number);
            keys.insert(key.clone());
            sqlx::query("DELETE FROM tracks WHERE file_id=? AND cue_path IS NULL")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO tracks(track_key,file_id,path,path_norm,dir_norm,ext,mtime_ms,size_bytes,meta_scanned_ms,title,artist,album,album_artist,artists_json,album_artists_json,artist_names_version,disc_number,track_number,duration_ms,cue_path,cue_track_number,start_cd_frame,end_cd_frame,start_frame,end_frame,segment_sample_rate) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,1,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(track_key) DO UPDATE SET file_id=excluded.file_id,path=excluded.path,path_norm=excluded.path_norm,dir_norm=excluded.dir_norm,mtime_ms=excluded.mtime_ms,size_bytes=excluded.size_bytes,meta_scanned_ms=excluded.meta_scanned_ms,title=excluded.title,artist=excluded.artist,album=excluded.album,album_artist=excluded.album_artist,artists_json=excluded.artists_json,album_artists_json=excluded.album_artists_json,disc_number=excluded.disc_number,track_number=excluded.track_number,duration_ms=excluded.duration_ms,start_cd_frame=excluded.start_cd_frame,end_cd_frame=excluded.end_cd_frame,start_frame=excluded.start_frame,end_frame=excluded.end_frame,segment_sample_rate=excluded.segment_sample_rate")
                .bind(key).bind(id).bind(&track.file).bind(&track.file).bind(parent_dir_norm(&track.file).unwrap_or_default()).bind(row.get::<&str,_>("ext"))
                .bind(row.get::<i64,_>("mtime_ms")).bind(row.get::<i64,_>("size_bytes")).bind(now_ms()).bind(title).bind(artist).bind(document.sheet.title.as_deref().or(metadata.album.as_deref())).bind(album_artist).bind(artists).bind(album_artists)
                .bind(document.sheet.disc_number.or(metadata.disc_number)).bind(track.number).bind(((u128::from(end-start)*1000)/u128::from(rate)) as i64)
                .bind(&path).bind(track.number).bind(track.index as i64).bind(end_cd.map(|v| v as i64)).bind(start as i64).bind(end as i64).bind(rate).execute(&mut *tx).await?;
        }
        let existing: Vec<String> =
            sqlx::query_scalar("SELECT track_key FROM tracks WHERE cue_path=?")
                .bind(&path)
                .fetch_all(&mut *tx)
                .await?;
        for key in existing {
            if !keys.contains(&key) {
                sqlx::query("DELETE FROM tracks WHERE track_key=?")
                    .bind(key)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        changed = true;
    }
    for id in restore {
        let active: i64 = sqlx::query_scalar("SELECT count(*) FROM tracks WHERE file_id=?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if active == 0 {
            restore_file(&mut tx, id, &roots, &excluded).await?;
        }
    }
    tx.commit().await?;
    Ok(changed)
}

async fn restore_file(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    id: i64,
    roots: &[String],
    excluded: &[String],
) -> Result<()> {
    let Some(row) = sqlx::query("SELECT * FROM audio_files WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
    else {
        return Ok(());
    };
    let path: &str = row.get("path");
    if !Path::new(path).is_file() || !beneath(path, roots) || beneath(path, excluded) {
        return Ok(());
    }
    let meta: ExtractedMetadata =
        serde_json::from_str(row.get::<Option<&str>, _>("metadata_json").unwrap_or("{}"))?;
    let artists = crate::artist_names::normalize_artists_json(
        &serde_json::to_string(&meta.artists)?,
        meta.artist.as_deref(),
    )?;
    let album_artists =
        crate::artist_names::normalize_artists_json("[]", meta.album_artist.as_deref())?;
    sqlx::query("INSERT INTO tracks(track_key,file_id,path,path_norm,dir_norm,ext,mtime_ms,size_bytes,meta_scanned_ms,title,artist,album,album_artist,artists_json,album_artists_json,artist_names_version,disc_number,track_number,duration_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,1,?,?,?)")
        .bind(format!("file:{}", row.get::<&str,_>("path_norm"))).bind(id).bind(path).bind(row.get::<&str,_>("path_norm")).bind(parent_dir_norm(path).unwrap_or_default()).bind(row.get::<&str,_>("ext"))
        .bind(row.get::<i64,_>("mtime_ms")).bind(row.get::<i64,_>("size_bytes")).bind(row.get::<i64,_>("meta_scanned_ms"))
        .bind(meta.title).bind(meta.artist).bind(meta.album).bind(meta.album_artist).bind(artists).bind(album_artists).bind(meta.disc_number).bind(meta.track_number).bind(meta.duration_ms).execute(&mut **tx).await?;
    Ok(())
}
pub(super) async fn covered(pool: &SqlitePool, path: &str) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM tracks WHERE path_norm=? AND cue_path IS NOT NULL",
    )
    .bind(normalize_path_str(path))
    .fetch_one(pool)
    .await?
        > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deleting_cue_directory_restores_audio_outside_that_directory() {
        let dir = tempfile::tempdir().unwrap();
        let pool = super::super::db::init_db(&dir.path().join("library.db"))
            .await
            .unwrap();
        wav(&dir.path().join("album.wav"));
        let root = normalized(dir.path()).unwrap();
        sqlx::query("INSERT INTO scan_roots(path) VALUES(?)")
            .bind(&root)
            .execute(&pool)
            .await
            .unwrap();
        let nested = dir.path().join("sheets");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(
            nested.join("album.cue"),
            "FILE \"../album.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00",
        )
        .unwrap();
        let hub = Arc::new(EventHub::new());
        super::super::watch::apply_fs_changes(
            &pool,
            &hub,
            dir.path(),
            &[],
            vec![nested.to_string_lossy().into_owned()],
            &None,
        )
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM tracks WHERE start_frame IS NOT NULL"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        std::fs::remove_file(nested.join("album.cue")).unwrap();
        std::fs::remove_dir(&nested).unwrap();
        super::super::watch::apply_fs_changes(
            &pool,
            &hub,
            dir.path(),
            &[],
            vec![nested.to_string_lossy().into_owned()],
            &None,
        )
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks WHERE start_frame IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn failed_first_import_keeps_whole_file_and_watch_reconciles_edits() {
        let dir = tempfile::tempdir().unwrap();
        let pool = super::super::db::init_db(&dir.path().join("library.db"))
            .await
            .unwrap();
        wav(&dir.path().join("album.wav"));
        let root = normalized(dir.path()).unwrap();
        sqlx::query("INSERT INTO scan_roots(path) VALUES(?)")
            .bind(&root)
            .execute(&pool)
            .await
            .unwrap();
        let path = dir.path().join("album.cue");
        std::fs::write(
            &path,
            "FILE \"album.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 99:00:00",
        )
        .unwrap();
        let hub = Arc::new(EventHub::new());
        super::super::scan::scan_all(&pool, &hub, dir.path(), false, &None)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks WHERE start_frame IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        std::fs::write(&path,"FILE \"album.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:01:01").unwrap();
        assert!(
            super::super::watch::apply_fs_changes(
                &pool,
                &hub,
                dir.path(),
                &[],
                vec![path.to_string_lossy().into_owned()],
                &None
            )
            .await
            .unwrap()
        );
        let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(ids.len(), 2);
        super::super::scan::scan_all(&pool, &hub, dir.path(), true, &None)
            .await
            .unwrap();
        assert_eq!(
            ids,
            sqlx::query_scalar::<_, i64>("SELECT id FROM tracks ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap()
        );
        std::fs::remove_file(&path).unwrap();
        assert!(
            super::super::watch::apply_fs_changes(
                &pool,
                &hub,
                dir.path(),
                &[],
                vec![path.to_string_lossy().into_owned()],
                &None
            )
            .await
            .unwrap()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks WHERE start_frame IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        pool.close().await;
    }

    #[tokio::test]
    #[ignore = "Read-only acceptance against an explicitly supplied real music root"]
    async fn real_music_cue_import_read_only() {
        let root = std::env::var("STELLATUNE_CUE_TEST_ROOT").expect("set STELLATUNE_CUE_TEST_ROOT");
        let dir = tempfile::tempdir().unwrap();
        let pool = super::super::db::init_db(&dir.path().join("acceptance.db"))
            .await
            .unwrap();
        sqlx::query("INSERT INTO scan_roots(path) VALUES(?)")
            .bind(&root)
            .execute(&pool)
            .await
            .unwrap();
        let hub = Arc::new(EventHub::new());
        let mut events = hub.subscribe();
        reconcile(
            &pool,
            &hub,
            dir.path(),
            std::slice::from_ref(&root),
            true,
            false,
            true,
        )
        .await
        .unwrap();
        while let Ok(event) = events.try_recv() {
            eprintln!("{event:?}");
        }
        let counts = sqlx::query("SELECT (SELECT count(*) FROM cue_documents) documents,(SELECT count(*) FROM audio_files) files,(SELECT count(*) FROM tracks WHERE start_frame IS NOT NULL) segments").fetch_one(&pool).await.unwrap();
        eprintln!(
            "CUE acceptance: documents={}, files={}, segments={}",
            counts.get::<i64, _>("documents"),
            counts.get::<i64, _>("files"),
            counts.get::<i64, _>("segments")
        );
        assert!(counts.get::<i64, _>("segments") > 0);
        let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert!(
            !reconcile(&pool, &hub, dir.path(), &[root], true, false, true)
                .await
                .unwrap()
        );
        assert_eq!(
            ids,
            sqlx::query_scalar::<_, i64>("SELECT id FROM tracks ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap()
        );
        pool.close().await;
    }

    fn wav(path: &Path) {
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend(192036_u32.to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16_u32.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(1_u16.to_le_bytes());
        bytes.extend(48000_u32.to_le_bytes());
        bytes.extend(96000_u32.to_le_bytes());
        bytes.extend(2_u16.to_le_bytes());
        bytes.extend(16_u16.to_le_bytes());
        bytes.extend(b"data");
        bytes.extend(192000_u32.to_le_bytes());
        bytes.resize(192044, 0);
        std::fs::write(path, bytes).unwrap();
    }
    #[tokio::test]
    async fn imports_atomically_preserves_ids_and_restores_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let pool = super::super::db::init_db(&dir.path().join("library.db"))
            .await
            .unwrap();
        wav(&dir.path().join("album.wav"));
        let root = normalized(dir.path()).unwrap();
        sqlx::query("INSERT INTO scan_roots(path) VALUES(?)")
            .bind(&root)
            .execute(&pool)
            .await
            .unwrap();
        let covers = dir.path().join("covers");
        std::fs::create_dir(&covers).unwrap();
        let cue = dir.path().join("album.cue");
        let text = "TITLE \"Album\"\nPERFORMER \"A / B\"\nFILE \"album.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"One\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nTITLE \"Two\"\nINDEX 01 00:01:01";
        std::fs::write(&cue, text).unwrap();
        let hub = Arc::new(EventHub::new());
        let sync = || {
            reconcile(
                &pool,
                &hub,
                &covers,
                std::slice::from_ref(&root),
                true,
                false,
                true,
            )
        };
        assert!(sync().await.unwrap());
        let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks ORDER BY track_number")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(ids.len(), 2);
        let boundary: i64 = sqlx::query_scalar("SELECT end_frame FROM tracks WHERE track_number=1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(boundary, 48640);
        assert!(!sync().await.unwrap());
        std::fs::write(&cue, text.replace("Two", "Second")).unwrap();
        assert!(sync().await.unwrap());
        let updated: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks ORDER BY track_number")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(updated, ids);
        std::fs::write(&cue, "FILE \"album.wav\" WAVE\nTRACK 01 AUDIO").unwrap();
        assert!(!sync().await.unwrap());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
        std::fs::remove_file(&cue).unwrap();
        assert!(sync().await.unwrap());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks WHERE cue_path IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        pool.close().await;
    }
    #[tokio::test]
    async fn duplicates_do_not_duplicate_tracks_and_conflicts_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        let pool = super::super::db::init_db(&dir.path().join("library.db"))
            .await
            .unwrap();
        wav(&dir.path().join("album.wav"));
        let root = normalized(dir.path()).unwrap();
        sqlx::query("INSERT INTO scan_roots(path) VALUES(?)")
            .bind(&root)
            .execute(&pool)
            .await
            .unwrap();
        let text = "FILE \"album.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:01:00";
        std::fs::write(dir.path().join("a.cue"), text).unwrap();
        std::fs::write(dir.path().join("b.cue"), text).unwrap();
        let hub = Arc::new(EventHub::new());
        reconcile(
            &pool,
            &hub,
            dir.path(),
            std::slice::from_ref(&root),
            true,
            false,
            true,
        )
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks")
                .fetch_one(&pool)
                .await
                .unwrap(),
            2
        );
        std::fs::write(
            dir.path().join("b.cue"),
            text.replace("00:01:00", "00:01:01"),
        )
        .unwrap();
        reconcile(&pool, &hub, dir.path(), &[root], true, false, true)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM tracks WHERE cue_path IS NULL")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        pool.close().await;
    }
}
