use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use sqlx::SqlitePool;
use walkdir::WalkDir;

use stellatune_core::LibraryEvent;

use crate::service::EventHub;

use super::Plugins;
use super::metadata::{extract_metadata_with_plugins, write_cover_bytes};
use super::paths::{is_drive_root, is_under_excluded, normalize_path_str, now_ms, parent_dir_norm};
use super::tracks::{select_track_fingerprint, upsert_track};

fn is_audio_ext(ext: &str) -> bool {
    matches!(ext, "mp3" | "flac" | "wav")
}

struct FileCandidate {
    path: String,
    path_norm: String,
    dir_norm: String,
    ext: String,
    mtime_ms: i64,
    size_bytes: i64,
}

pub(super) async fn scan_all(
    pool: &SqlitePool,
    events: &Arc<EventHub>,
    cover_dir: &PathBuf,
    plugins: &Plugins,
    force: bool,
) -> Result<()> {
    let roots: Vec<String> = sqlx::query_scalar!("SELECT path FROM scan_roots WHERE enabled=1")
        .fetch_all(pool)
        .await?;

    let excluded: Vec<String> = sqlx::query_scalar("SELECT path FROM excluded_folders")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|p: String| normalize_path_str(&p))
        .filter(|p| !p.is_empty())
        .collect();

    let started = Instant::now();
    let scan_started_ms = now_ms();
    let mut scanned: u64 = 0;
    let mut upserted: u64 = 0;
    let mut skipped: u64 = 0;
    let mut errors: u64 = 0;

    for root in roots {
        events.emit(LibraryEvent::Log {
            message: format!("scanning: {root}"),
        });

        let (tx, mut rx) = tokio::sync::mpsc::channel::<FileCandidate>(512);
        let root_clone = root.clone();
        let excluded_clone = excluded.clone();
        let plugins_for_walk = plugins.clone();
        let plugins_for_meta = plugins.clone();

        // Blocking filesystem enumeration & metadata.
        let walker = tokio::task::spawn_blocking(move || {
            for entry in WalkDir::new(&root_clone).follow_links(false).into_iter() {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path().to_path_buf();
                let path_str = path.to_string_lossy().to_string();
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let supported = is_audio_ext(&ext) || {
                    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
                    {
                        if ext.is_empty() {
                            plugins_for_walk
                                .lock()
                                .ok()
                                .and_then(|pm| pm.can_decode_path(&path_str).ok())
                                .unwrap_or(false)
                        } else {
                            plugins_for_walk
                                .lock()
                                .ok()
                                .map(|pm| pm.probe_best_decoder_hint(&ext).is_some())
                                .unwrap_or(false)
                        }
                    }
                    #[cfg(not(any(
                        target_os = "windows",
                        target_os = "linux",
                        target_os = "macos"
                    )))]
                    {
                        false
                    }
                };
                if !supported {
                    continue;
                }
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let mtime_ms = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                let size_bytes = meta.len() as i64;
                let path_norm = normalize_path_str(&path_str);
                let dir_norm = parent_dir_norm(&path_norm).unwrap_or_default();
                if !dir_norm.is_empty() && is_under_excluded(&dir_norm, &excluded_clone) {
                    continue;
                }
                if tx
                    .blocking_send(FileCandidate {
                        path: path_str,
                        path_norm,
                        dir_norm,
                        ext,
                        mtime_ms,
                        size_bytes,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        while let Some(file) = rx.recv().await {
            scanned += 1;

            if !force {
                // Skip unchanged.
                if let Some(old) = select_track_fingerprint(pool, &file.path).await? {
                    if old.mtime_ms == file.mtime_ms
                        && old.size_bytes == file.size_bytes
                        && old.meta_scanned_ms > 0
                    {
                        skipped += 1;
                        continue;
                    }
                }
            }

            let meta_scanned_ms = now_ms();

            let plugins = plugins_for_meta.clone();
            let (title, artist, album, duration_ms, cover) = match tokio::task::spawn_blocking({
                let path = file.path.clone();
                move || {
                    extract_metadata_with_plugins(Path::new(&path), &plugins)
                        .map(|m| (m.title, m.artist, m.album, m.duration_ms, m.cover))
                }
            })
            .await
            {
                Ok(Ok(m)) => m,
                Ok(Err(e)) => {
                    errors += 1;
                    events.emit(LibraryEvent::Log {
                        message: format!("metadata error: {}: {e:#}", file.path),
                    });
                    (None, None, None, None, None)
                }
                Err(join_err) => {
                    errors += 1;
                    events.emit(LibraryEvent::Log {
                        message: format!("metadata task failed: {}: {join_err}", file.path),
                    });
                    (None, None, None, None, None)
                }
            };

            let track_id = match upsert_track(
                pool,
                &file.path,
                &file.ext,
                file.mtime_ms,
                file.size_bytes,
                title.as_deref(),
                artist.as_deref(),
                album.as_deref(),
                duration_ms,
                meta_scanned_ms,
                &file.path_norm,
                &file.dir_norm,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    errors += 1;
                    events.emit(LibraryEvent::Log {
                        message: format!("upsert error: {}: {e}", file.path),
                    });
                    continue;
                }
            };

            if let Some(bytes) = cover {
                if let Err(e) = write_cover_bytes(cover_dir, track_id, &bytes) {
                    errors += 1;
                    events.emit(LibraryEvent::Log {
                        message: format!("cover write error: {}: {e}", file.path),
                    });
                }
            }

            upserted += 1;

            if scanned % 500 == 0 {
                events.emit(LibraryEvent::ScanProgress {
                    scanned: scanned as i64,
                    updated: upserted as i64,
                    skipped: skipped as i64,
                    errors: errors as i64,
                });
            }
        }

        if let Err(join_err) = walker.await {
            errors += 1;
            events.emit(LibraryEvent::Log {
                message: format!("walk task failed: {join_err}"),
            });
        }

        sqlx::query!(
            "UPDATE scan_roots SET last_scan_ms=?1 WHERE path=?2",
            scan_started_ms,
            root
        )
        .execute(pool)
        .await?;
    }

    events.emit(LibraryEvent::ScanProgress {
        scanned: scanned as i64,
        updated: upserted as i64,
        skipped: skipped as i64,
        errors: errors as i64,
    });
    events.emit(LibraryEvent::ScanFinished {
        duration_ms: started.elapsed().as_millis() as i64,
        scanned: scanned as i64,
        updated: upserted as i64,
        skipped: skipped as i64,
        errors: errors as i64,
    });

    Ok(())
}

pub(super) async fn scan_folder_into_db(
    pool: SqlitePool,
    events: &Arc<EventHub>,
    cover_dir: &PathBuf,
    plugins: &Plugins,
    folder_norm: &str,
) -> Result<bool> {
    let folder_norm = normalize_path_str(folder_norm);
    if folder_norm.is_empty() || is_drive_root(&folder_norm) {
        return Ok(false);
    }
    let root = PathBuf::from(&folder_norm);
    if !root.exists() {
        return Ok(false);
    }

    let excluded: Vec<String> = sqlx::query_scalar("SELECT path FROM excluded_folders")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|p: String| normalize_path_str(&p))
        .filter(|p| !p.is_empty())
        .collect();

    let mut changed = false;

    for entry in WalkDir::new(&root).follow_links(false).into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path().to_path_buf();
        let path_str = path.to_string_lossy().to_string();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let supported = is_audio_ext(&ext) || {
            #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
            {
                match plugins.lock() {
                    Ok(pm) => {
                        if ext.is_empty() {
                            pm.can_decode_path(&path_str).unwrap_or(false)
                        } else {
                            pm.probe_best_decoder_hint(&ext).is_some()
                        }
                    }
                    Err(_) => false,
                }
            }
            #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
            {
                false
            }
        };
        if !supported {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let size_bytes = meta.len() as i64;

        let path_norm = normalize_path_str(&path_str);
        let dir_norm = parent_dir_norm(&path_norm).unwrap_or_default();
        if !dir_norm.is_empty() && is_under_excluded(&dir_norm, &excluded) {
            continue;
        }

        if let Some(old) = select_track_fingerprint(&pool, &path_str).await? {
            if old.mtime_ms == mtime_ms && old.size_bytes == size_bytes && old.meta_scanned_ms > 0 {
                continue;
            }
        }

        let meta_scanned_ms = now_ms();

        let plugins = plugins.clone();
        let (title, artist, album, duration_ms, cover) = match tokio::task::spawn_blocking({
            let p = path.clone();
            move || {
                extract_metadata_with_plugins(&p, &plugins)
                    .map(|m| (m.title, m.artist, m.album, m.duration_ms, m.cover))
            }
        })
        .await
        {
            Ok(Ok(m)) => m,
            Ok(Err(e)) => {
                events.emit(LibraryEvent::Log {
                    message: format!("metadata error: {}: {e:#}", path_str),
                });
                (None, None, None, None, None)
            }
            Err(join_err) => {
                events.emit(LibraryEvent::Log {
                    message: format!("metadata task failed: {}: {join_err}", path_str),
                });
                (None, None, None, None, None)
            }
        };

        let track_id = upsert_track(
            &pool,
            &path_str,
            &ext,
            mtime_ms,
            size_bytes,
            title.as_deref(),
            artist.as_deref(),
            album.as_deref(),
            duration_ms,
            meta_scanned_ms,
            &path_norm,
            &dir_norm,
        )
        .await?;

        if let Some(bytes) = cover {
            if let Err(e) = write_cover_bytes(cover_dir, track_id, &bytes) {
                events.emit(LibraryEvent::Log {
                    message: format!("cover write error: {}: {e}", path_str),
                });
            }
        }

        changed = true;
    }

    Ok(changed)
}
