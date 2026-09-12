use crate::metadata_provider::MetadataProvider;
use anyhow::Result;
use sqlx::{Row, SqlitePool};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
use stellatune_media_probe::{PROBE_VERSION, ProbeResult, ProbeStatus};

pub(super) fn fingerprint(path: &Path) -> std::io::Result<(i64, i64)> {
    let stat = std::fs::metadata(path)?;
    let mtime = stat
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    Ok((mtime, stat.len() as i64))
}

pub(super) fn matches(path: &Path, mtime: i64, size: i64) -> bool {
    fingerprint(path).is_ok_and(|f| f == (mtime, size))
}

/// Properties-only refresh. Writes neither decoder sample geometry nor track metadata.
pub(super) async fn refresh(
    pool: &SqlitePool,
    path: &Path,
    provider: &Option<Arc<dyn MetadataProvider>>,
    force: bool,
    retry_io: bool,
) -> Result<bool> {
    let norm = super::paths::normalize_path_str(&path.to_string_lossy());
    let Some(row) = sqlx::query("SELECT id,mtime_ms,size_bytes,probe_version,probe_status,probe_mtime_ms,probe_size_bytes FROM audio_files WHERE path_norm=?")
        .bind(&norm).fetch_optional(pool).await? else { return Ok(false); };
    let mtime: i64 = row.get("mtime_ms");
    let size: i64 = row.get("size_bytes");
    let status: Option<String> = row.get("probe_status");
    if !force
        && row.get::<i64, _>("probe_version") == PROBE_VERSION
        && row.get::<Option<i64>, _>("probe_mtime_ms") == Some(mtime)
        && row.get::<Option<i64>, _>("probe_size_bytes") == Some(size)
        && !(retry_io && matches!(status.as_deref(), Some("ioError" | "cancelled")))
    {
        return Ok(false);
    }
    if !matches(path, mtime, size) {
        return Ok(false);
    }
    let owned = path.to_owned();
    let provider = provider.clone();
    let result = tokio::task::spawn_blocking(move || {
        if let Some(provider) = provider.filter(|p| p.supports(&owned)) {
            return provider.probe_audio(&owned);
        }
        match std::fs::File::open(&owned) {
            Ok(file) => stellatune_media_probe::probe(
                file,
                owned.extension().and_then(|e| e.to_str()),
                &AtomicBool::new(false),
            ),
            Err(_) => ProbeResult {
                properties: None,
                status: ProbeStatus::IoError,
                bytes_read: 0,
            },
        }
    })
    .await?;
    if !matches(path, mtime, size) {
        return Ok(false);
    }
    if result.status != ProbeStatus::Ready {
        tracing::debug!(path = %path.display(), status = ?result.status, bytes_read = result.bytes_read, "audio properties unavailable");
    }
    let status = serde_json::to_value(result.status)?
        .as_str()
        .unwrap()
        .to_owned();
    let json = result
        .properties
        .map(|p| serde_json::to_string(&p))
        .transpose()?;
    let changed = sqlx::query("UPDATE audio_files SET properties_json=?,probe_version=?,probe_status=?,probe_mtime_ms=?,probe_size_bytes=? WHERE id=? AND mtime_ms=? AND size_bytes=?")
        .bind(json).bind(PROBE_VERSION).bind(status).bind(mtime).bind(size).bind(row.get::<i64,_>("id")).bind(mtime).bind(size).execute(pool).await?.rows_affected();
    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Provider {
        count: AtomicUsize,
        io_failure: bool,
        mutate: bool,
    }
    impl MetadataProvider for Provider {
        fn supports(&self, _: &Path) -> bool {
            true
        }
        fn inspect(&self, _: &Path) -> Result<crate::metadata_provider::LocalFileMetadata> {
            panic!("properties upgrade must not read tags or artwork")
        }
        fn inspect_audio(
            &self,
            path: &Path,
        ) -> Result<Option<stellatune_media_probe::AudioProperties>> {
            self.count.fetch_add(1, Ordering::Relaxed);
            if self.mutate {
                std::fs::write(path, b"changed during probe")?;
            }
            anyhow::ensure!(!self.io_failure, "temporary IO error");
            Ok(None)
        }
    }
    async fn setup() -> (tempfile::TempDir, SqlitePool, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.ncm");
        std::fs::write(&path, b"sample").unwrap();
        let (mtime, size) = fingerprint(&path).unwrap();
        let pool = super::super::db::init_db(&dir.path().join("library.db"))
            .await
            .unwrap();
        let norm = super::super::paths::normalize_path_str(&path.to_string_lossy());
        sqlx::query("INSERT INTO audio_files(id,path,path_norm,ext,mtime_ms,size_bytes,sample_rate,total_frames,pcm_bits) VALUES(42,?,?,'ncm',?,?,96000,960000,24)")
            .bind(&norm).bind(&norm).bind(mtime).bind(size).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO tracks(id,file_id,path,mtime_ms,size_bytes,title,start_frame,end_frame,segment_sample_rate) VALUES(7,42,?,0,0,'Retained title',0,96000,96000),(8,42,?,0,0,'Next segment',96000,192000,96000)")
            .bind(&norm).bind(&norm).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO playlist_tracks(playlist_id,track_id) SELECT id,7 FROM playlists WHERE system_key='liked'").execute(&pool).await.unwrap();
        (dir, pool, path)
    }
    #[tokio::test]
    async fn properties_upgrade_is_shared_cached_and_preserves_track_and_sample_identity() {
        let (_dir, pool, path) = setup().await;
        let provider = Arc::new(Provider {
            count: AtomicUsize::new(0),
            io_failure: false,
            mutate: false,
        });
        let p: Option<Arc<dyn MetadataProvider>> = Some(provider.clone());
        assert!(refresh(&pool, &path, &p, false, true).await.unwrap());
        assert!(!refresh(&pool, &path, &p, false, true).await.unwrap());
        assert_eq!(provider.count.load(Ordering::Relaxed), 1);
        let tracks: Vec<(i64, String, i64, i64)> =
            sqlx::query_as("SELECT id,title,start_frame,end_frame FROM tracks ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            tracks,
            [
                (7, "Retained title".into(), 0, 96000),
                (8, "Next segment".into(), 96000, 192000)
            ]
        );
        let geometry: (i64, i64, i64) =
            sqlx::query_as("SELECT sample_rate,total_frames,pcm_bits FROM audio_files WHERE id=42")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(geometry, (96000, 960000, 24));
        let likes: i64 =
            sqlx::query_scalar("SELECT count(*) FROM playlist_tracks WHERE track_id=7")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(likes, 1);
        assert!(refresh(&pool, &path, &p, true, true).await.unwrap());
        sqlx::query("UPDATE audio_files SET probe_version=0")
            .execute(&pool)
            .await
            .unwrap();
        assert!(refresh(&pool, &path, &p, false, true).await.unwrap());
        assert_eq!(provider.count.load(Ordering::Relaxed), 3);
    }
    #[tokio::test]
    async fn io_failure_retries_only_on_explicit_scan_and_stale_results_are_discarded() {
        let (_dir, pool, path) = setup().await;
        let provider = Arc::new(Provider {
            count: AtomicUsize::new(0),
            io_failure: true,
            mutate: false,
        });
        let p: Option<Arc<dyn MetadataProvider>> = Some(provider.clone());
        assert!(refresh(&pool, &path, &p, false, true).await.unwrap());
        assert!(!refresh(&pool, &path, &p, false, false).await.unwrap());
        assert!(refresh(&pool, &path, &p, false, true).await.unwrap());
        assert_eq!(provider.count.load(Ordering::Relaxed), 2);
        let p: Option<Arc<dyn MetadataProvider>> = Some(Arc::new(Provider {
            count: AtomicUsize::new(0),
            io_failure: false,
            mutate: true,
        }));
        assert!(!refresh(&pool, &path, &p, true, true).await.unwrap());
        let status: String = sqlx::query_scalar("SELECT probe_status FROM audio_files WHERE id=42")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "ioError");
    }

    #[tokio::test]
    async fn all_scan_paths_use_installed_codecs_and_upgrade_without_metadata_rescan() {
        for method in 0..3 {
            let dir = tempfile::tempdir().unwrap();
            let music = dir.path().join("music");
            std::fs::create_dir(&music).unwrap();
            let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../stellatune-media-probe/tests/fixtures");
            let names = [
                "cbr.mp3",
                "adts.aac",
                "aac.m4a",
                "alac.m4a",
                "tone.flac",
                "vorbis.ogg",
                "float.caf",
                "integer.aiff",
            ];
            for name in names.into_iter().chain(["opus.ogg"]) {
                std::fs::copy(fixtures.join(name), music.join(name)).unwrap();
            }
            std::fs::copy(fixtures.join("aac.m4a"), music.join("video.mp4")).unwrap();
            let pool = super::super::db::init_db(&dir.path().join("library.db"))
                .await
                .unwrap();
            let norm = super::super::paths::normalize_path_str(&music.to_string_lossy());
            sqlx::query("INSERT INTO scan_roots(path) VALUES(?)")
                .bind(&norm)
                .execute(&pool)
                .await
                .unwrap();
            let hub = Arc::new(crate::service::EventHub::new());
            let covers = dir.path().join("covers");
            match method {
                0 => super::super::scan::scan_all(&pool, &hub, &covers, false, &None)
                    .await
                    .unwrap(),
                1 => {
                    super::super::scan::scan_folder_into_db(
                        pool.clone(),
                        &hub,
                        &covers,
                        &norm,
                        &None,
                        true,
                    )
                    .await
                    .unwrap();
                },
                _ => {
                    super::super::watch::apply_fs_changes(
                        &pool,
                        &hub,
                        &covers,
                        &[],
                        std::fs::read_dir(&music)
                            .unwrap()
                            .map(|e| e.unwrap().path().to_string_lossy().into_owned())
                            .collect(),
                        &None,
                    )
                    .await
                    .unwrap();
                },
            }
            let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
            assert_eq!(ids.len(), names.len(), "scan method {method}");
            let ready: i64 =
                sqlx::query_scalar("SELECT count(*) FROM audio_files WHERE probe_status='ready'")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(ready, names.len() as i64);
            // Deliberately change a cached title; a technical upgrade must leave it alone.
            sqlx::query("UPDATE tracks SET title='Keep my title'")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE audio_files SET probe_version=0,properties_json=NULL")
                .execute(&pool)
                .await
                .unwrap();
            super::super::scan::scan_all(&pool, &hub, &covers, false, &None)
                .await
                .unwrap();
            let after: Vec<i64> =
                sqlx::query_scalar("SELECT id FROM tracks WHERE title='Keep my title' ORDER BY id")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
            assert_eq!(after, ids);
        }
    }
}
