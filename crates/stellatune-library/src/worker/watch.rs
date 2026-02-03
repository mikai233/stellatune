use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use stellatune_core::LibraryEvent;

use crate::service::EventHub;

use super::metadata::{extract_metadata, write_cover_bytes};
use super::paths::{is_under_excluded, normalize_path_str, now_ms, parent_dir_norm};
use super::tracks::{
    delete_track_by_path_norm, select_track_fingerprint_by_path_norm, upsert_track_by_path_norm,
};

#[derive(Debug, Clone, Copy)]
pub(super) enum WatchCtrl {
    Refresh,
}

async fn refresh_watch_state(
    pool: &SqlitePool,
    watcher: &mut RecommendedWatcher,
    watched: &mut HashSet<String>,
    excluded: &mut Vec<String>,
) -> Result<()> {
    let roots: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT path
        FROM scan_roots
        WHERE enabled=1
        "#,
    )
    .fetch_all(pool)
    .await?;

    let desired = roots
        .into_iter()
        .map(|p| normalize_path_str(&p))
        .filter(|p| !p.is_empty())
        .collect::<HashSet<_>>();

    *excluded = sqlx::query_scalar("SELECT path FROM excluded_folders")
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|p: String| normalize_path_str(&p))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();

    for r in desired.iter() {
        if watched.contains(r) {
            continue;
        }
        watcher
            .watch(Path::new(r), RecursiveMode::Recursive)
            .with_context(|| format!("watch failed: {r}"))?;
        watched.insert(r.clone());
    }

    let to_remove = watched
        .iter()
        .filter(|p| !desired.contains(*p))
        .cloned()
        .collect::<Vec<_>>();
    for r in to_remove {
        let _ = watcher.unwatch(Path::new(&r));
        watched.remove(&r);
    }

    Ok(())
}

pub(super) fn spawn_watch_task(
    pool: SqlitePool,
    events: Arc<EventHub>,
    cover_dir: PathBuf,
) -> mpsc::UnboundedSender<WatchCtrl> {
    let (ctrl_tx, mut ctrl_rx) = mpsc::unbounded_channel::<WatchCtrl>();

    tokio::spawn(async move {
        let (fs_tx, mut fs_rx) = mpsc::unbounded_channel::<notify::Result<notify::Event>>();

        let mut watcher = match notify::recommended_watcher(move |res| {
            let _ = fs_tx.send(res);
        }) {
            Ok(w) => w,
            Err(e) => {
                events.emit(LibraryEvent::Log {
                    message: format!("fs watcher init failed: {e}"),
                });
                return;
            }
        };

        let mut watched = HashSet::<String>::new();
        let mut excluded = Vec::<String>::new();
        let mut dirty = HashSet::<String>::new();
        let mut debounce: Option<tokio::time::Instant> = None;

        if let Err(e) = refresh_watch_state(&pool, &mut watcher, &mut watched, &mut excluded).await
        {
            events.emit(LibraryEvent::Log {
                message: format!("fs watcher refresh failed: {e:#}"),
            });
        }

        loop {
            let sleep = async {
                if let Some(t) = debounce {
                    tokio::time::sleep_until(t).await;
                } else {
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                }
            };

            tokio::select! {
                Some(ctrl) = ctrl_rx.recv() => {
                    match ctrl {
                        WatchCtrl::Refresh => {
                            if let Err(e) = refresh_watch_state(&pool, &mut watcher, &mut watched, &mut excluded).await {
                                events.emit(LibraryEvent::Log {
                                    message: format!("fs watcher refresh failed: {e:#}"),
                                });
                            }
                        }
                    }
                }
                Some(res) = fs_rx.recv() => {
                    let ev = match res {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    for p in ev.paths {
                        let raw = p.to_string_lossy().to_string();
                        if !raw.trim().is_empty() {
                            dirty.insert(raw);
                        }
                    }
                    debounce = Some(tokio::time::Instant::now() + Duration::from_millis(750));
                }
                _ = sleep => {
                    if dirty.is_empty() {
                        debounce = None;
                        continue;
                    }
                    let batch = dirty.drain().collect::<Vec<_>>();
                    debounce = None;

                    match apply_fs_changes(&pool, &events, &cover_dir, &excluded, batch).await {
                        Ok(true) => events.emit(LibraryEvent::Changed),
                        Ok(false) => {}
                        Err(e) => events.emit(LibraryEvent::Log { message: format!("fs sync error: {e:#}") }),
                    }
                }
            }
        }
    });

    ctrl_tx
}

fn is_audio_ext(ext: &str) -> bool {
    matches!(ext, "mp3" | "flac" | "wav")
}

async fn apply_fs_changes(
    pool: &SqlitePool,
    events: &Arc<EventHub>,
    cover_dir: &PathBuf,
    excluded: &[String],
    raw_paths: Vec<String>,
) -> Result<bool> {
    let mut changed = false;

    for raw in raw_paths {
        let raw_trimmed = raw.trim();
        if raw_trimmed.is_empty() {
            continue;
        }

        let path = PathBuf::from(raw_trimmed);
        let path_norm = normalize_path_str(raw_trimmed);
        if path_norm.is_empty() {
            continue;
        }

        if std::fs::metadata(&path).ok().is_some_and(|m| m.is_dir()) {
            continue;
        }

        let dir_norm = parent_dir_norm(&path_norm).unwrap_or_default();
        if !dir_norm.is_empty() && is_under_excluded(&dir_norm, excluded) {
            continue;
        }

        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                let deleted = delete_track_by_path_norm(pool, cover_dir, &path_norm).await?;
                changed |= deleted > 0;
                continue;
            }
        };

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !is_audio_ext(&ext) {
            let deleted = delete_track_by_path_norm(pool, cover_dir, &path_norm).await?;
            changed |= deleted > 0;
            continue;
        }

        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let size_bytes = meta.len() as i64;

        if let Some(old) = select_track_fingerprint_by_path_norm(pool, &path_norm).await? {
            if old.mtime_ms == mtime_ms && old.size_bytes == size_bytes && old.meta_scanned_ms > 0 {
                continue;
            }
        }

        // Heavy metadata extraction happens only when the fingerprint differs.
        let meta_scanned_ms = now_ms();

        let (title, artist, album, duration_ms, cover) = match tokio::task::spawn_blocking({
            let p = path.clone();
            move || extract_metadata(&p)
        })
        .await
        {
            Ok(Ok(m)) => (m.title, m.artist, m.album, m.duration_ms, m.cover),
            Ok(Err(e)) => {
                events.emit(LibraryEvent::Log {
                    message: format!("metadata error: {}: {e:#}", raw_trimmed),
                });
                (None, None, None, None, None)
            }
            Err(join_err) => {
                events.emit(LibraryEvent::Log {
                    message: format!("metadata task failed: {}: {join_err}", raw_trimmed),
                });
                (None, None, None, None, None)
            }
        };

        let track_id = upsert_track_by_path_norm(
            pool,
            raw_trimmed,
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
                    message: format!("cover write error: {}: {e}", raw_trimmed),
                });
            }
        }

        changed = true;
    }

    Ok(changed)
}
